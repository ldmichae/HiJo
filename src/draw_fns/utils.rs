use embassy_nrf::{peripherals::TWISPI0, twim::Twim};
use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Text},
};
use nmea::sentences::FixType;
use ssd1306::{
    Ssd1306, mode::BufferedGraphicsMode, prelude::I2CInterface, size::DisplaySize128x64,
};

use crate::{TEXT_STYLE_SM, draw_fns, gps::reader::GpsReaderResults, utils::float::FloatToString};

pub fn draw_static_text<D>(display: &mut D, lg: MonoTextStyle<BinaryColor>) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor>,
{
    Text::with_alignment("HIJO", Point::new(64, 12), lg, Alignment::Center).draw(display)?;

    Ok(())
}

pub fn draw_optional_float<D>(
    display: &mut D,
    value: Option<impl Into<f64>>,
    x: i32,
    y: i32,
    style: MonoTextStyle<BinaryColor>,
) where
    D: DrawTarget<Color = BinaryColor>,
{
    if let Some(v) = value {
        let mut float_buf = FloatToString::new();
        let text = float_buf.convert(v.into());
        let _ = Text::new(text, Point::new(x, y), style).draw(display);
    }
}

pub fn draw_coords(
    last_lat_lon_alt: &Option<GpsReaderResults>,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, TWISPI0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    if let Some(lat_lon_alt) = &last_lat_lon_alt {
        draw_fns::utils::draw_optional_float(display, lat_lon_alt.lat, 0, 32, TEXT_STYLE_SM);
        draw_fns::utils::draw_optional_float(display, lat_lon_alt.lon, 0, 40, TEXT_STYLE_SM);
        draw_fns::utils::draw_optional_float(display, lat_lon_alt.alt, 0, 48, TEXT_STYLE_SM);
    }
}

pub fn draw_fix_status(
    last_fix: Option<FixType>,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, TWISPI0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    if let Some(fix) = &last_fix {
        let fix_text = match fix {
            FixType::Invalid => "INVALID",
            FixType::Gps => "GPS",
            FixType::DGps => "DGPS",
            _ => "OTHER",
        };
        Text::new(fix_text, Point::new(4, 60), TEXT_STYLE_SM)
            .draw(display)
            .unwrap();
    } else {
        Text::new("NO GPS", Point::new(4, 60), TEXT_STYLE_SM)
            .draw(display)
            .unwrap();
    }
}

pub fn draw_recording_status(
    is_recording: bool,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, TWISPI0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    let recording_state_text = if is_recording { "STOP" } else { "START" };
    Text::new(recording_state_text, Point::new(72, 60), TEXT_STYLE_SM)
        .draw(display)
        .unwrap();
}

pub fn draw_moving_jo(
    x: i32,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, TWISPI0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    Text::new("hijo", Point::new(x, 52), TEXT_STYLE_SM)
        .draw(display)
        .unwrap();
}

pub fn draw_total_distance(
    distance_raw: f64,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, TWISPI0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    draw_optional_float(display, Some(distance_raw), 80, 60, TEXT_STYLE_SM);
}

pub fn draw_current_speed(
    speed_raw: f64,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, TWISPI0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    draw_optional_float(display, Some(speed_raw), 80, 48, TEXT_STYLE_SM);
}

pub fn draw_last_segment_distance(
    distance_raw: f64,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, TWISPI0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    draw_optional_float(display, Some(distance_raw), 80, 40, TEXT_STYLE_SM);
}

pub fn draw_hdop(
    distance_raw: f32,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, TWISPI0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    draw_optional_float(display, Some(distance_raw), 80, 32, TEXT_STYLE_SM);
}
