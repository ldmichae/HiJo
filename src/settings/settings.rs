use crate::utils::vector::CircularTracker;

#[derive(Copy, Clone, Debug)]
pub struct Setting<T: Copy + Default> {
    pub id: &'static u8,
    pub label: &'static str,
    pub options: CircularTracker<8, (&'static str, T)>,
}

#[derive(Copy, Clone, Default, Debug)]
pub enum SettingsWrapper {
    #[default]
    Default,
    Bool(Setting<bool>),
    Text(Setting<&'static str>),
    AnyNumber(Setting<isize>),
}

pub type SettingsState = CircularTracker<3, SettingsWrapper>;
