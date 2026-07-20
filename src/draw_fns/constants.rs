use embedded_graphics::{
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_4X6, FONT_5X8, FONT_6X13, FONT_10X20},
    },
    pixelcolor::BinaryColor,
};

pub const TEXT_STYLE_LG: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
pub const TEXT_STYLE_MD: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_6X13, BinaryColor::On);
pub const TEXT_STYLE_SM: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
pub const TEXT_STYLE_XS: MonoTextStyle<'_, BinaryColor> =
    MonoTextStyle::new(&FONT_4X6, BinaryColor::On);
