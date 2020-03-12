// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;

use directories::ProjectDirs;
use yaml_rust::yaml::{Yaml, YamlLoader};

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
    pub(crate) fuzzy_match_color: Color,
    pub(crate) fuzzy_select_color: Color,
    pub(crate) fuzzy_select_match_color: Color,
    pub(crate) fuzzy_select_background_color: Color,
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
            fuzzy_match_color: Color::new(255, 100, 0, 255), // FIXME
            fuzzy_select_color: Color::new(255, 100, 0, 255), // FIXME
            fuzzy_select_match_color: Color::new(255, 100, 0, 255), // FIXME
            fuzzy_select_background_color: Color::new(0, 0, 0, 8),
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
        // Try loading config
        ProjectDirs::from("", "sbarua", "bed")
            .and_then(|proj_dirs| {
                let cfg_dir_path = proj_dirs.config_dir();
                read_to_string(cfg_dir_path.join("config.yml")).ok()
            })
            .and_then(|data| YamlLoader::load_from_str(&data).ok())
            .and_then(|docs| Cfg::from_yaml(&docs[0], font_core))
            .unwrap_or_else(|| {
                // Otherwise, return default config
                let default_theme = CfgTheme::default(font_core);
                let mut themes = HashMap::new();
                themes.insert("default".to_owned(), default_theme);
                Cfg {
                    themes: themes,
                    cur_theme: "default".to_owned(),
                }
            })
    }

    pub(crate) fn theme(&self) -> &CfgTheme {
        self.themes.get(&self.cur_theme).unwrap()
    }

    fn from_yaml(yaml: &Yaml, fc: &mut FontCore) -> Option<Cfg> {
        // Parse themes
        let mut themes = HashMap::new();
        let yaml_themes = &yaml["themes"].as_hash()?;
        for (key, val) in yaml_themes.iter() {
            let theme_name = key.as_str()?;
            let theme = CfgTheme {
                // Textview
                textview_background_color: Color::parse(
                    val["textview_background_color"].as_str()?,
                )?,
                textview_foreground_color: Color::parse(
                    val["textview_foreground_color"].as_str()?,
                )?,
                textview_cursor_color: Color::parse(val["textview_cursor_color"].as_str()?)?,
                textview_text_size: TextSize::from_f32(val["textview_text_size"].as_f64()? as f32),
                textview_fixed_face: fc.find(val["textview_fixed_face"].as_str()?)?,
                textview_variable_face: fc.find(val["textview_variable_face"].as_str()?)?,
                // Gutter
                gutter_background_color: Color::parse(val["gutter_background_color"].as_str()?)?,
                gutter_foreground_color: Color::parse(val["gutter_foreground_color"].as_str()?)?,
                gutter_text_size: TextSize::from_f32(val["gutter_text_size"].as_f64()? as f32),
                gutter_fixed_face: fc.find(val["gutter_fixed_face"].as_str()?)?,
                gutter_variable_face: fc.find(val["gutter_variable_face"].as_str()?)?,
                gutter_padding: val["gutter_padding"].as_i64()? as u32,
                // Prompt
                fuzzy_background_color: Color::parse(val["fuzzy_background_color"].as_str()?)?,
                fuzzy_foreground_color: Color::parse(val["fuzzy_foreground_color"].as_str()?)?,
                fuzzy_label_color: Color::parse(val["fuzzy_label_color"].as_str()?)?,
                fuzzy_match_color: Color::parse(val["fuzzy_match_color"].as_str()?)?,
                fuzzy_select_color: Color::parse(val["fuzzy_select_color"].as_str()?)?,
                fuzzy_select_match_color: Color::parse(val["fuzzy_select_match_color"].as_str()?)?,
                fuzzy_select_background_color: Color::parse(
                    val["fuzzy_select_background_color"].as_str()?,
                )?,
                fuzzy_cursor_color: Color::parse(val["fuzzy_cursor_color"].as_str()?)?,
                fuzzy_text_size: TextSize::from_f32(val["fuzzy_text_size"].as_f64()? as f32),
                fuzzy_face: fc.find(val["fuzzy_face"].as_str()?)?,
                fuzzy_max_height_percentage: val["fuzzy_max_height_percentage"].as_i64()? as u32,
                fuzzy_width_percentage: val["fuzzy_width_percentage"].as_i64()? as u32,
                fuzzy_edge_padding: val["fuzzy_edge_padding"].as_i64()? as u32,
                fuzzy_line_spacing: val["fuzzy_line_spacing"].as_i64()? as u32,
                fuzzy_bottom_offset: val["fuzzy_bottom_offset"].as_i64()? as u32,
            };
            themes.insert(theme_name.to_owned(), theme);
        }
        // Get current theme
        let cur_theme = yaml["theme"].as_str()?;
        if themes.contains_key(cur_theme) {
            Some(Cfg {
                themes: themes,
                cur_theme: cur_theme.to_owned(),
            })
        } else {
            None
        }
    }
}
