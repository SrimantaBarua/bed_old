// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::collections::HashMap;

use crate::font::{FaceKey, FontCore};
use crate::types::{Color, TextSize};

#[cfg(target_os = "linux")]
const FIXED_FONT: &'static str = "monospace";
#[cfg(target_os = "windows")]
const FIXED_FONT: &'static str = "Consolas";

#[cfg(target_os = "linux")]
const VARIABLE_FONT: &'static str = "sans";
#[cfg(target_os = "windows")]
const VARIABLE_FONT: &'static str = "Arial";

#[derive(Clone)]
pub(crate) struct CfgTheme {
    // Textview
    pub(crate) textview_background_color: Color,
    pub(crate) textview_foreground_color: Color,
    pub(crate) textview_cursor_color: Color,
    pub(crate) textview_text_size: TextSize,
    pub(crate) textview_fixed_face: FaceKey,
    pub(crate) textview_variable_face: FaceKey,
    // Gutter
    pub(crate) gutter_background_color: Color,
    pub(crate) gutter_foreground_color: Color,
    pub(crate) gutter_text_size: TextSize,
    pub(crate) gutter_fixed_face: FaceKey,
    pub(crate) gutter_variable_face: FaceKey,
    pub(crate) gutter_padding: u32,
    // Prompt
    pub(crate) fuzzy_background_color: Color,
    pub(crate) fuzzy_foreground_color: Color,
    pub(crate) fuzzy_label_color: Color,
    pub(crate) fuzzy_select_color: Color,
    pub(crate) fuzzy_cursor_color: Color,
    pub(crate) fuzzy_text_size: TextSize,
    pub(crate) fuzzy_face: FaceKey,
    pub(crate) fuzzy_max_height_percentage: u32,
    pub(crate) fuzzy_width_percentage: u32,
    pub(crate) fuzzy_edge_padding: u32,
    pub(crate) fuzzy_line_spacing: u32,
    pub(crate) fuzzy_bottom_offset: u32,
}

impl CfgTheme {
    fn default(fc: &mut FontCore) -> CfgTheme {
        // Get default fixed and variable width fonts
        let (fixed_face, variable_face) = {
            let fixed_face = fc.find(FIXED_FONT).expect("failed to get monospace font");
            let variable_face = fc.find(VARIABLE_FONT).expect("failed to get sans font");
            (fixed_face, variable_face)
        };
        CfgTheme {
            textview_background_color: Color::new(255, 255, 255, 255),
            textview_foreground_color: Color::new(96, 96, 96, 255),
            textview_cursor_color: Color::new(255, 128, 0, 196),
            textview_text_size: TextSize::from_f32(8.0),
            textview_fixed_face: fixed_face,
            textview_variable_face: variable_face,
            gutter_background_color: Color::new(255, 255, 255, 255),
            gutter_foreground_color: Color::new(196, 196, 196, 255),
            gutter_text_size: TextSize::from_f32(7.0),
            gutter_fixed_face: fixed_face,
            gutter_variable_face: variable_face,
            gutter_padding: 10,
            fuzzy_background_color: Color::new(255, 255, 255, 255),
            fuzzy_foreground_color: Color::new(144, 144, 144, 255),
            fuzzy_label_color: Color::new(96, 96, 96, 255),
            fuzzy_select_color: Color::new(255, 100, 0, 255),
            fuzzy_cursor_color: Color::new(255, 128, 0, 196),
            fuzzy_text_size: TextSize::from_f32(8.0),
            fuzzy_face: variable_face,
            fuzzy_max_height_percentage: 40,
            fuzzy_width_percentage: 90,
            fuzzy_edge_padding: 10,
            fuzzy_line_spacing: 2,
            fuzzy_bottom_offset: 10,
        }
    }
}

pub(crate) struct Cfg {
    // Themes
    themes: HashMap<String, CfgTheme>,
    cur_theme: String,
}

impl Cfg {
    pub(crate) fn load(font_core: &mut FontCore) -> Cfg {
        let default_theme = CfgTheme::default(font_core);
        let mut themes = HashMap::new();
        themes.insert("default".to_owned(), default_theme);
        Cfg {
            themes: themes,
            cur_theme: "default".to_owned(),
        }
    }

    pub(crate) fn theme(&self) -> &CfgTheme {
        self.themes.get(&self.cur_theme).unwrap()
    }
}
