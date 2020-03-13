// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::collections::HashMap;
use std::fs::read_to_string;

use directories::ProjectDirs;
use yaml_rust::yaml::{Yaml, YamlLoader};

use crate::font::{FaceKey, FontCore};
use crate::types::{Color, TextSize, TextSlant, TextStyle, TextWeight};

#[cfg(target_os = "linux")]
const FIXED_FONT: &'static str = "monospace";
#[cfg(target_os = "windows")]
const FIXED_FONT: &'static str = "Consolas";

#[cfg(target_os = "linux")]
const VARIABLE_FONT: &'static str = "sans";
#[cfg(target_os = "windows")]
const VARIABLE_FONT: &'static str = "Arial";

#[derive(Clone)]
pub(crate) struct CfgThemeSyntaxElem {
    pub(crate) foreground_color: Color,
    pub(crate) style: TextStyle,
}

#[derive(Clone, Default)]
pub(crate) struct CfgThemeSyntax {
    pub(crate) comment: Option<CfgThemeSyntaxElem>,
    pub(crate) accessor: Option<CfgThemeSyntaxElem>,
    pub(crate) operator: Option<CfgThemeSyntaxElem>,
    pub(crate) separator: Option<CfgThemeSyntaxElem>,
    pub(crate) keyword: Option<CfgThemeSyntaxElem>,
    pub(crate) identifier: Option<CfgThemeSyntaxElem>,
    pub(crate) string: Option<CfgThemeSyntaxElem>,
    pub(crate) number: Option<CfgThemeSyntaxElem>,
}

#[derive(Clone)]
pub(crate) struct CfgThemeUI {
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

#[derive(Clone)]
pub(crate) struct CfgTheme {
    pub(crate) ui: CfgThemeUI,
    pub(crate) syntax: CfgThemeSyntax,
}

impl CfgTheme {
    fn default(fc: &mut FontCore) -> CfgTheme {
        // Get default fixed and variable width fonts
        let (fixed_face, variable_face) = {
            let fixed_face = fc.find(FIXED_FONT).expect("failed to get monospace font");
            let variable_face = fc.find(VARIABLE_FONT).expect("failed to get sans font");
            (fixed_face, variable_face)
        };
        let ui = CfgThemeUI {
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
        };
        let syntax = CfgThemeSyntax::default();
        CfgTheme {
            ui: ui,
            syntax: syntax,
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
        for (key, theme_val) in yaml_themes.iter() {
            let theme_name = key.as_str()?;
            // Get UI theme
            let val = &theme_val["ui"];
            let ui = CfgThemeUI {
                // Textview
                textview_background_color: yaml_color(val, "textview_background_color")?,
                textview_foreground_color: yaml_color(val, "textview_foreground_color")?,
                textview_cursor_color: yaml_color(val, "textview_cursor_color")?,
                textview_text_size: yaml_textsize(val, "textview_text_size")?,
                textview_fixed_face: yaml_face(val, fc, "textview_fixed_face")?,
                textview_variable_face: yaml_face(val, fc, "textview_variable_face")?,
                // Gutter
                gutter_background_color: yaml_color(val, "gutter_background_color")?,
                gutter_foreground_color: yaml_color(val, "gutter_foreground_color")?,
                gutter_text_size: yaml_textsize(val, "gutter_text_size")?,
                gutter_fixed_face: yaml_face(val, fc, "gutter_fixed_face")?,
                gutter_variable_face: yaml_face(val, fc, "gutter_variable_face")?,
                gutter_padding: val["gutter_padding"].as_i64()? as u32,
                // Prompt
                fuzzy_background_color: yaml_color(val, "fuzzy_background_color")?,
                fuzzy_foreground_color: yaml_color(val, "fuzzy_foreground_color")?,
                fuzzy_label_color: yaml_color(val, "fuzzy_label_color")?,
                fuzzy_match_color: yaml_color(val, "fuzzy_match_color")?,
                fuzzy_select_color: yaml_color(val, "fuzzy_select_color")?,
                fuzzy_select_match_color: yaml_color(val, "fuzzy_select_match_color")?,
                fuzzy_select_background_color: yaml_color(val, "fuzzy_select_background_color")?,
                fuzzy_cursor_color: yaml_color(val, "fuzzy_cursor_color")?,
                fuzzy_text_size: yaml_textsize(val, "fuzzy_text_size")?,
                fuzzy_face: yaml_face(val, fc, "fuzzy_face")?,
                fuzzy_max_height_percentage: val["fuzzy_max_height_percentage"].as_i64()? as u32,
                fuzzy_width_percentage: val["fuzzy_width_percentage"].as_i64()? as u32,
                fuzzy_edge_padding: val["fuzzy_edge_padding"].as_i64()? as u32,
                fuzzy_line_spacing: val["fuzzy_line_spacing"].as_i64()? as u32,
                fuzzy_bottom_offset: val["fuzzy_bottom_offset"].as_i64()? as u32,
            };
            // Get syntax elements
            let val = &theme_val["syntax"];
            let syntax = CfgThemeSyntax {
                comment: yaml_syntax_elem(&val["comment"]),
                operator: yaml_syntax_elem(&val["operator"]),
                accessor: yaml_syntax_elem(&val["accessor"]),
                separator: yaml_syntax_elem(&val["separator"]),
                keyword: yaml_syntax_elem(&val["keyword"]),
                identifier: yaml_syntax_elem(&val["identifier"]),
                string: yaml_syntax_elem(&val["string"]),
                number: yaml_syntax_elem(&val["number"]),
            };
            let theme = CfgTheme {
                ui: ui,
                syntax: syntax,
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

fn yaml_syntax_elem(yaml: &Yaml) -> Option<CfgThemeSyntaxElem> {
    let slant = yaml["text_slant"]
        .as_str()
        .and_then(|s| TextSlant::from_str(s))
        .unwrap_or_default();
    let weight = yaml["text_weight"]
        .as_str()
        .and_then(|s| TextWeight::from_str(s))
        .unwrap_or_default();
    yaml["foreground_color"]
        .as_str()
        .and_then(|s| Color::parse(s))
        .map(|fgcol| CfgThemeSyntaxElem {
            foreground_color: fgcol,
            style: TextStyle::new(weight, slant),
        })
}

fn yaml_textsize(yaml: &Yaml, elem: &str) -> Option<TextSize> {
    yaml[elem].as_f64().map(|f| TextSize::from_f32(f as f32))
}

fn yaml_color(yaml: &Yaml, elem: &str) -> Option<Color> {
    yaml[elem].as_str().and_then(|s| Color::parse(s))
}

fn yaml_face(yaml: &Yaml, font_core: &mut FontCore, elem: &str) -> Option<FaceKey> {
    let faces = yaml[elem].as_str()?;
    for face_name in faces.split(',') {
        if let Some(key) = font_core.find(face_name) {
            return Some(key);
        }
    }
    None
}
