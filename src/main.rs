#![no_std]
#![no_main]

use core::panic::PanicInfo;
use cortex_m_rt::entry;
use nrf52840_hal as hal;

use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, ascii::FONT_6X13_ITALIC, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Text},
};
use hal::{
    gpio::p0,
    pac::Peripherals,
    twim::{Frequency, Pins, Twim},
};
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};

#[entry]
fn main() -> ! {
    let p = Peripherals::take().unwrap();
    let port0 = p0::Parts::new(p.P0);

    // Choose two unused GPIOs for SDA and SCL
    let sda = port0.p0_22.into_floating_input().degrade(); // D2
    let scl = port0.p0_24.into_floating_input().degrade(); // D3

    // Set up I2C bus
    let i2c = Twim::new(p.TWIM0, Pins { scl, sda }, Frequency::K100);

    // Create display interface
    let interface = I2CDisplayInterface::new(i2c);

    // Initialize display
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    display.init().unwrap();

    // Create text style
    let text_style_lg = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
    let text_style_italic = MonoTextStyle::new(&FONT_6X13_ITALIC, BinaryColor::On);

    // Draw centered "Hello!" text
    Text::with_alignment(
        "Hi, Jo :)",
        Point::new(64, 12),
        text_style_lg,
        Alignment::Center,
    )
    .draw(&mut display)
    .unwrap();

    // Draw centered "Hello!" text
    Text::with_alignment(
        "It's MAMA!",
        Point::new(64, 32),
        text_style_italic,
        Alignment::Center,
    )
    .draw(&mut display)
    .unwrap();

    // Flush the display buffer to screen
    display.flush().unwrap();

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
