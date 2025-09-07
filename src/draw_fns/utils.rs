use core::fmt::Write;
use embassy_nrf::{peripherals::SERIAL0, twim::Twim};
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
use heapless::{String};

use crate::{draw_fns, gps::reader::{GpsReaderResults, InternalDateDTO}, utils::float::FloatToString, TEXT_STYLE_MD, TEXT_STYLE_SM, TEXT_STYLE_XS};

pub fn draw_static_text<D>(display: &mut D, style: MonoTextStyle<BinaryColor>) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor>,
{
    Text::with_alignment("HIJO", Point::new(90, 12), style, Alignment::Center).draw(display)?;

    Ok(())
}

pub fn draw_optional_float<D>(
    prefix: Option<&str>,
    suffix: Option<&str>,
    precision: u8,
    display: &mut D,
    value: Option<impl Into<f64>>,
    x: i32,
    y: i32,
    style: MonoTextStyle<BinaryColor>,
) where
    D: DrawTarget<Color = BinaryColor>,
{
    if let Some(v) = value {
        let mut float_buf = FloatToString::new(precision);
        let float_convert = float_buf.convert(v.into());
        let mut text: String<32> = String::new();

        if let Some(pre) = prefix {
            let _ = text.push_str(pre);
            let _ = text.push_str(" ");
        }

        let _ = text.push_str(float_convert);

        if let Some(suf) = suffix {
            let _ = text.push_str(suf);
        }
        let _ = Text::new(&text, Point::new(x, y), style).draw(display);
    }
}

pub fn draw_coords(
    last_lat_lon_alt: &Option<GpsReaderResults>,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, SERIAL0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    if let Some(lat_lon_alt) = &last_lat_lon_alt {
        draw_fns::utils::draw_optional_float(None, None, 6, display, lat_lon_alt.lat, 0, 32, TEXT_STYLE_XS);
        draw_fns::utils::draw_optional_float(None, None, 6, display, lat_lon_alt.lon, 0, 38, TEXT_STYLE_XS);
        draw_fns::utils::draw_optional_float(None, None, 6, display, lat_lon_alt.alt, 0, 44, TEXT_STYLE_XS);
    }
}

pub fn draw_recording_status(
    is_recording: bool,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, SERIAL0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    let recording_state_text = if is_recording { ">>" } else { "--" };
    Text::new(recording_state_text, Point::new(0, 22), TEXT_STYLE_SM)
        .draw(display)
        .unwrap();
}

pub fn draw_blinky(
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, SERIAL0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    Text::new(".", Point::new(0, 63), TEXT_STYLE_SM)
        .draw(display)
        .unwrap();
}

pub fn draw_total_distance(
    distance_raw: f64,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, SERIAL0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    let mut drawable_distance: f64 = distance_raw;
    let mut drawable_precision: u8 = 0;
    let mut drawable_unit: &str = "'";
    if distance_raw > 5280.0 {
        drawable_distance = distance_raw / 5280.0;
        drawable_precision = 3;
        drawable_unit = "mi.";
    }
    draw_optional_float(Some(">"), Some(drawable_unit), drawable_precision, display, Some(drawable_distance), 70, 60, TEXT_STYLE_SM);
}

pub fn draw_total_elev_gain(
    gain_raw: f64,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, SERIAL0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    draw_optional_float(Some("^"), Some("'"), 0, display, Some(gain_raw), 70, 50, TEXT_STYLE_SM);
}

pub fn draw_current_speed(
    speed_raw: f64,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, SERIAL0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    draw_optional_float(None, Some("mph"), 2, display, Some(speed_raw), 70, 36, TEXT_STYLE_MD);
}

pub fn draw_last_segment_distance(
    distance_raw: f64,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, SERIAL0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    draw_optional_float(None, Some("ft"), 1, display, Some(distance_raw), 70, 40, TEXT_STYLE_SM);
}

pub fn draw_hdop(
    fix: Option<FixType>, // This is the change: it's now an Option<FixType>
    hdop_raw: f32,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, SERIAL0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    let mut quality_text ;

    match fix {
        Some(FixType::Invalid) => {
            quality_text = "X";
        }
        Some(FixType::Gps) => {
            quality_text = "O";
            if hdop_raw < 2.0 { quality_text = "O)" };
            if hdop_raw < 1.0 { quality_text = "O))" };
        },
        Some(FixType::DGps) => {
            quality_text = "D";
            if hdop_raw < 2.0 { quality_text = "D)" };
            if hdop_raw < 1.0 { quality_text = "D))" };
        },
        Some(FixType::FloatRtk) => {
            quality_text = "R";
            if hdop_raw < 1.0 { quality_text = "R)" }; // Good RTK Float (HDOP still matters)
        },
        Some(FixType::Rtk) => quality_text = "R))", // RTK Fixed is usually the best, HDOP might still be provided but less critical

        // Handle the None case for the Option<FixType>
        None => {
            quality_text = "N/A"; // Or "NoFix", "---", " " to indicate no fix data at all
        },
        // For any other unhandled FixType that might be inside Some()
        Some(_) => quality_text = "??",
    }
        Text::new(quality_text, Point::new(110, 8), TEXT_STYLE_SM)
            .draw(display)
            .unwrap();
}

pub fn draw_datetime(
    dto: &Option<InternalDateDTO>,
    display: &mut Ssd1306<
        I2CInterface<Twim<'_, SERIAL0>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >
) {
    if let Some(dto_verified) = dto {
        let mut full_str = heapless::String::<19>::new();
        let _ = write!(full_str, "{} {}", dto_verified.pretty_date.as_str(), dto_verified.pretty_time.as_str());
        Text::new(&full_str.as_str(), Point::new(0, 8), TEXT_STYLE_XS)
        .draw(display)
        .unwrap();
    }
}