use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;
use nrf52840_hal::{self as hal, Timer};

use hal::gpio;

pub fn blinky() -> ! {
    let p = hal::pac::Peripherals::take().unwrap();
    let port0 = gpio::p0::Parts::new(p.P0);

    let mut led = port0.p0_09.into_push_pull_output(gpio::Level::Low);

    let mut timer = Timer::new(p.TIMER0);

    loop {
        led.set_high().unwrap();
        timer.delay_ms(2000_u32); // delay 500 milliseconds

        led.set_low().unwrap();
        timer.delay_ms(2000_u32);
    }
}
