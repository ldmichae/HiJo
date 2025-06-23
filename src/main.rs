#![no_std]
#![no_main]

mod blinky;
mod gps;
mod uart;

use crate::{
    gps::Gps,
    uart::{GpsUart, init_uart},
};
use core::panic::PanicInfo;
use cortex_m_rt::entry;
use nrf52840_hal::{self as hal};

use embedded_graphics::{
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X13_ITALIC, FONT_10X20},
        iso_8859_15::FONT_5X8,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Text},
};
use hal::{
    gpio::p0,
    pac::Peripherals,
    twim::{Frequency, Pins, Twim},
};
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

fn draw_static_text<D>(
    display: &mut D,
    lg: MonoTextStyle<BinaryColor>,
    italic: MonoTextStyle<BinaryColor>,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor>,
{
    Text::with_alignment("HIJO", Point::new(64, 12), lg, Alignment::Center).draw(display)?;

    Text::with_alignment("gps", Point::new(64, 32), italic, Alignment::Center).draw(display)?;

    Ok(())
}

#[entry]
fn main() -> ! {
    let p = Peripherals::take().unwrap();
    let uarte0 = p.UARTE0;

    let port0_parts = p0::Parts::new(p.P0);

    let p0_06 = port0_parts.p0_06; // UART TX (MCU)
    let p0_08 = port0_parts.p0_08; // UART RX (MCU)
    let p0_22 = port0_parts.p0_22; // I2C SDA
    let p0_24 = port0_parts.p0_24; // I2C SCL

    let sda = p0_22.into_floating_input().degrade();
    let scl = p0_24.into_floating_input().degrade();

    let i2c = Twim::new(p.TWIM0, Pins { scl, sda }, Frequency::K100);
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().unwrap();

    let text_style_lg = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
    let text_style_sm = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
    let text_style_italic = MonoTextStyle::new(&FONT_6X13_ITALIC, BinaryColor::On);

    let uart: GpsUart = init_uart(p0_06, p0_08, uarte0);
    let mut gps = Gps::init(uart);

    let mut x = 128;

    loop {
        display.clear(BinaryColor::Off).unwrap();

        draw_static_text(&mut display, text_style_lg, text_style_italic).unwrap();

        Text::new("hijo", Point::new(x, 52), text_style_sm)
            .draw(&mut display)
            .unwrap();

        let gps_parse = gps.read_and_parse().unwrap();

        if let Some(_fix) = gps_parse.fix {
            Text::new("GPS OK", Point::new(4, 60), text_style_sm)
                .draw(&mut display)
                .unwrap();
        } else {
            Text::new("NO GPS", Point::new(4, 60), text_style_sm)
                .draw(&mut display)
                .unwrap();
        }

        Text::new(&gps_parse.line, Point::new(0, 48), text_style_sm)
            .draw(&mut display)
            .unwrap();

        display.flush().unwrap();

        x = if x <= -24 { 128 } else { x - 1 };

        cortex_m::asm::delay(8000);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
