#![no_std]
#![no_main]

use vl53l4cd::{
    consts::VL53L4CD_DEFAULT_I2C_ADDRESS,
    Vl53l4cd, 
    ResultsData,
    bus_operation::Vl53l4cdI2C
};

use panic_halt as _; 
use cortex_m_rt::entry;

use core::{fmt::Write, cell::RefCell};

use embedded_hal::i2c::SevenBitAddress;

use stm32f4xx_hal::{
    gpio::{
        Output, 
        Pin, 
        PinState::High,
        gpioa, 
        gpiob,
        Alternate}, 
    pac::{USART2, Peripherals, CorePeripherals, TIM1}, 
    prelude::*, 
    serial::{Config, Tx}, 
    timer::{Delay, SysDelay},
    rcc::{Rcc, Clocks}
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

fn take_inst(sensor: &mut Vl53l4cd<Vl53l4cdI2C<RefCellDevice<StmI2c<I2C1>>>, Pin<'B', 3, Output>, Delay<TIM1, 1000>>, tx: &mut Tx<USART2>) {
    while !sensor.check_data_ready().unwrap() {} // Wait for data to be ready
    sensor.clear_interrupt().unwrap();
    let results = sensor.get_ranging_data().unwrap(); // Get and parse the result data
    write_results(tx, &results); // Print the result to the output
}

#[entry]
fn main() -> ! {
    let mut results: ResultsData;
    
    let dp: Peripherals = Peripherals::take().unwrap();
    let cp: CorePeripherals = CorePeripherals::take().unwrap();
    let rcc: Rcc = dp.RCC.constrain();
    let clocks: Clocks = rcc.cfgr.use_hse(8.MHz()).sysclk(48.MHz()).freeze();
    let _delay: SysDelay = cp.SYST.delay(&clocks);
    let tim_top: Delay<TIM1, 1000> = dp.TIM1.delay_ms(&clocks);


    let gpioa: gpioa::Parts = dp.GPIOA.split();
    let gpiob: gpiob::Parts = dp.GPIOB.split();
    
    let xshut_pin: Pin<'B', 3, Output> = gpiob.pb3.into_push_pull_output_in_state(High);
    let tx_pin: Pin<'A', 2, Alternate<7>> = gpioa.pa2.into_alternate();
     
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
        
    let mut sensor_top: Vl53l4cd<Vl53l4cdI2C<RefCellDevice<StmI2c<I2C1>>>, Pin<'B', 3, Output>, Delay<TIM1, 1000>> = Vl53l4cd::new_i2c(
        RefCellDevice::new(&i2c_bus),  
            xshut_pin,
            tim_top
        ).unwrap();

    sensor_top.init_sensor(address).unwrap(); 
    sensor_top.set_range_timing(10, 0).unwrap();
    sensor_top.start_ranging().unwrap();

    loop {
        take_inst(&mut sensor_top, &mut tx);
    }

} 
