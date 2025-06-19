#![no_main]
#![no_std]

use core::panic::PanicInfo;
use cortex_m_rt::entry;
use nrf52840_hal::twim::Frequency;
use nrf52840_hal::{self as hal, twim::Pins};
use nrf52840_hal::Twim

use hal::{gpio, prelude::*, timer::Timer};

#[entry]
fn main() -> ! {
    let p = hal::pac::Peripherals::take().unwrap();
    let port0 = gpio::p0::Parts::new(p.P0);

    let sda = port0.p0_24.into_floating_input().degrade();
    let scl = port0.p0_22.into_floating_input().degrade();

    let i2c = Twim::new(p.TWIM0, Pins { scl, sda }, Frequency::K100);

    let mut led = port0.p0_09.into_push_pull_output(gpio::Level::Low);

    let mut timer = Timer::new(p.TIMER0);

    loop {
        led.set_high().unwrap();
        timer.delay_ms(2000_u32); // delay 500 milliseconds

        led.set_low().unwrap();
        timer.delay_ms(2000_u32);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
