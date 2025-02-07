#![no_std]
#![no_main]

use vl53l4cd::{
    accessors::{DetectionThresholds, ThresholdWindow}, consts::VL53L4CD_DEFAULT_I2C_ADDRESS, ResultsData, Vl53l4cd
};

use panic_halt as _; 
use cortex_m::interrupt::Mutex;
use cortex_m_rt::entry;

use core::{fmt::Write, cell::RefCell};

use embedded_hal::i2c::SevenBitAddress;

use stm32f4xx_hal::{
    gpio::{self, gpioa, gpiob, Alternate, Edge, Input, Output, Pin,             PinState::High}, 
    pac::{interrupt, CorePeripherals, Peripherals, TIM1, USART2}, 
    rcc::{Clocks, Rcc}, 
    serial::{Config, Tx}, 
    timer::{Delay, SysDelay},
    prelude::*, 
};

// I2C related imports
use stm32f4xx_hal::{
    pac::I2C1,
    i2c::{I2c as StmI2c, I2c1, Mode}};
use embedded_hal_bus::i2c::RefCellDevice;

fn write_results(tx: &mut Tx<USART2>, results: &ResultsData) {

    writeln!(tx, "\x1B[2H").unwrap();

    writeln!(tx, "VL53L4A1 Simple Ranging demo application\n").unwrap();
    writeln!(tx, "Status = {sta:>4}\r\n", 
        sta = results.range_status).unwrap();
    writeln!(tx, "Distance [mm] = {dis:>4}\r\n", 
        dis = results.distance_mm).unwrap();
    writeln!(tx, "Signal [kcps/spad] = {sig:>4}\r\n", 
        sig = results.signal_per_spad_kcps).unwrap();
}

static INT_PIN: Mutex<RefCell<Option<gpio::PA4<Input>>>> = Mutex::new(RefCell::new(None));


#[entry]
fn main() -> ! {
    let mut results: ResultsData;
    
    let mut dp: Peripherals = Peripherals::take().unwrap();
    let cp: CorePeripherals = CorePeripherals::take().unwrap();
    let rcc: Rcc = dp.RCC.constrain();
    let clocks: Clocks = rcc.cfgr.use_hse(8.MHz()).sysclk(48.MHz()).freeze();
    let tim_top: Delay<TIM1, 1000> = dp.TIM1.delay_ms(&clocks);
    let _delay: SysDelay = cp.SYST.delay(&clocks);

    let gpioa: gpioa::Parts = dp.GPIOA.split();
    let gpiob: gpiob::Parts = dp.GPIOB.split();
    
    let xshut_pin: Pin<'B', 3, Output> = gpiob.pb3.into_push_pull_output_in_state(High);

    let tx_pin: Pin<'A', 2, Alternate<7>> = gpioa.pa2.into_alternate();
    
    let mut int_pin: Pin<'A', 4> = gpioa.pa4.into_input().internal_pull_up(true);
    // Configure Pin for Interrupts
    // 1) Promote SYSCFG structure to HAL to be able to configure interrupts
    let mut syscfg = dp.SYSCFG.constrain();
    // 2) Make an interrupt source
    int_pin.make_interrupt_source(&mut syscfg);
    // 3) Make an interrupt source  
    int_pin.trigger_on_edge(&mut dp.EXTI, Edge::Falling);
    // 4) Enable gpio interrupt
    int_pin.enable_interrupt(&mut dp.EXTI);

    // Enable the external interrupt in the NVIC by passing the interrupt number
    unsafe {
        cortex_m::peripheral::NVIC::unmask(int_pin.interrupt());
    }

    // Now that pin is configured, move pin into global context
    cortex_m::interrupt::free(|cs| {
        INT_PIN.borrow(cs).replace(Some(int_pin));
    });

    
    let mut tx: Tx<USART2> = dp.USART2.tx(
        tx_pin,
        Config::default()
        .baudrate(460800.bps())
        .wordlength_8()
        .parity_none(),
        &clocks).unwrap();
    

    let scl: Pin<'B', 8> = gpiob.pb8;
    let sda: Pin<'B', 9> = gpiob.pb9;
    
    let i2c: StmI2c<I2C1> = I2c1::new(
        dp.I2C1,
        (scl, sda),
        Mode::Standard{frequency:200.kHz()},
        &clocks);
        
    let i2c_bus: RefCell<StmI2c<I2C1>> = RefCell::new(i2c);
    let address: SevenBitAddress = VL53L4CD_DEFAULT_I2C_ADDRESS;
        
    let mut sensor = Vl53l4cd::new_i2c(
        RefCellDevice::new(&i2c_bus), 
            xshut_pin,
            tim_top
        ).unwrap();

    sensor.init_sensor(address).unwrap(); 

    let mut thresholds: DetectionThresholds = DetectionThresholds::new(); 
   thresholds.distance_high_mm = 200;
   thresholds.distance_low_mm = 100;
   thresholds.window = ThresholdWindow::Out;

    sensor.set_detection_thresholds(thresholds).unwrap(); 

    let t = sensor.get_detection_thresholds().unwrap();
    assert_eq!(t.distance_high_mm, 200);
    assert_eq!(t.distance_low_mm, 100);
    assert_eq!(t.window as u8, ThresholdWindow::Out as u8);

    // sensor.set_range_timing(10, 0).unwrap();
    sensor.start_ranging().unwrap();
    
    loop {
        while !sensor.check_data_ready().unwrap() {} // Wait for data to be ready
        sensor.clear_interrupt().unwrap();
        results = sensor.get_ranging_data().unwrap(); // Get and parse the result data
        write_results(&mut tx, &results); // Print the result to the output
    }
} 

#[interrupt]
fn EXTI4() {
    // Start a Critical Section
    cortex_m::interrupt::free(|cs| {
        // Obtain Access to Global Data
        //INTERRUPT.borrow(cs).set(true);
        // Obtain access to Peripheral and Clear Interrupt Pending Flag
        let mut int_pin = INT_PIN.borrow(cs).borrow_mut();
        int_pin.as_mut().unwrap().clear_interrupt_pending_bit();
    });
}