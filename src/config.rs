// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::collections::HashMap;
use std::default::Default;
use std::fs::read_to_string;
use std::path::Path;

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

const TEXT_SIZE: f64 = 8.0;
const GUTTER_TEXT_SIZE: f64 = 7.0;

#[derive(Debug)]
pub(crate) struct CfgUiTextview {
    pub(crate) text_size: TextSize,
    pub(crate) fixed_face: FaceKey,
    pub(crate) variable_face: FaceKey,
}

impl CfgUiTextview {
    fn from_yaml(yaml: &Yaml, font_core: &mut FontCore) -> CfgUiTextview {
        let text_size = TextSize::from_f32(yaml["text_size"].as_f64().unwrap_or(TEXT_SIZE) as f32);
        let fixed_face_names = yaml["fixed_face"].as_str().unwrap_or(FIXED_FONT);
        let variable_face_names = yaml["variable_face"].as_str().unwrap_or(VARIABLE_FONT);
        let fixed_face =
            face_from_str(fixed_face_names, font_core).expect("failed to get fixed face");
        let variable_face =
            face_from_str(variable_face_names, font_core).expect("failed to get variable face");
        CfgUiTextview {
            text_size: text_size,
            fixed_face: fixed_face,
            variable_face: variable_face,
        }
    }

    fn default(fc: &mut FontCore) -> CfgUiTextview {
        let fixed = fc.find(FIXED_FONT).expect("failed to get fixed face");
        let variable = fc.find(VARIABLE_FONT).expect("failed to get variable face");
        CfgUiTextview {
            text_size: TextSize::from_f32(TEXT_SIZE as f32),
            fixed_face: fixed,
            variable_face: variable,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CfgUiGutter {
    pub(crate) text_size: TextSize,
    pub(crate) fixed_face: FaceKey,
    pub(crate) variable_face: FaceKey,
    pub(crate) padding: u32,
}

impl CfgUiGutter {
    fn from_yaml(yaml: &Yaml, font_core: &mut FontCore) -> CfgUiGutter {
        let text_size =
            TextSize::from_f32(yaml["text_size"].as_f64().unwrap_or(GUTTER_TEXT_SIZE) as f32);
        let fixed_face_names = yaml["fixed_face"].as_str().unwrap_or(FIXED_FONT);
        let variable_face_names = yaml["variable_face"].as_str().unwrap_or(VARIABLE_FONT);
        let fixed_face =
            face_from_str(fixed_face_names, font_core).expect("failed to get fixed face");
        let variable_face =
            face_from_str(variable_face_names, font_core).expect("failed to get variable face");
        let padding = yaml["padding"].as_i64().unwrap_or(10) as u32;
        CfgUiGutter {
            text_size: text_size,
            fixed_face: fixed_face,
            variable_face: variable_face,
            padding: padding,
        }
    }

    fn default(fc: &mut FontCore) -> CfgUiGutter {
        let fixed = fc.find(FIXED_FONT).expect("failed to get fixed face");
        let variable = fc.find(VARIABLE_FONT).expect("failed to get variable face");
        CfgUiGutter {
            text_size: TextSize::from_f32(GUTTER_TEXT_SIZE as f32),
            fixed_face: fixed,
            variable_face: variable,
            padding: 10,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CfgUiFuzzy {
    pub(crate) text_size: TextSize,
    pub(crate) fixed_face: FaceKey,
    pub(crate) variable_face: FaceKey,
    pub(crate) max_height_percentage: u32,
    pub(crate) width_percentage: u32,
    pub(crate) line_spacing: u32,
    pub(crate) bottom_offset: u32,
}

impl CfgUiFuzzy {
    fn from_yaml(yaml: &Yaml, font_core: &mut FontCore) -> CfgUiFuzzy {
        let text_size = TextSize::from_f32(yaml["text_size"].as_f64().unwrap_or(TEXT_SIZE) as f32);
        let fixed_face_names = yaml["fixed_face"].as_str().unwrap_or(FIXED_FONT);
        let variable_face_names = yaml["variable_face"].as_str().unwrap_or(VARIABLE_FONT);
        let fixed_face =
            face_from_str(fixed_face_names, font_core).expect("failed to get fixed face");
        let variable_face =
            face_from_str(variable_face_names, font_core).expect("failed to get variable face");
        let max_height_perc = yaml["max_height_percentage"].as_i64().unwrap_or(40) as u32;
        let width_perc = yaml["width_percentage"].as_i64().unwrap_or(85) as u32;
        let line_space = yaml["line_spacing"].as_i64().unwrap_or(1) as u32;
        let botoff = yaml["bottom_offset"].as_i64().unwrap_or(10) as u32;
        CfgUiFuzzy {
            text_size: text_size,
            fixed_face: fixed_face,
            variable_face: variable_face,
            max_height_percentage: max_height_perc,
            width_percentage: width_perc,
            line_spacing: line_space,
            bottom_offset: botoff,
        }
    }

    fn default(fc: &mut FontCore) -> CfgUiFuzzy {
        let fixed = fc.find(FIXED_FONT).expect("failed to get fixed face");
        let variable = fc.find(VARIABLE_FONT).expect("failed to get variable face");
        CfgUiFuzzy {
            text_size: TextSize::from_f32(GUTTER_TEXT_SIZE as f32),
            fixed_face: fixed,
            variable_face: variable,
            max_height_percentage: 40,
            width_percentage: 85,
            line_spacing: 1,
            bottom_offset: 10,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CfgUiThemeTextview {
    pub(crate) background_color: Color,
    pub(crate) foreground_color: Color,
    pub(crate) cursor_color: Color,
    pub(crate) cursor_text_color: Color,
}

impl Default for CfgUiThemeTextview {
    fn default() -> CfgUiThemeTextview {
        CfgUiThemeTextview {
            background_color: Color::new(255, 255, 255, 255),
            foreground_color: Color::new(0, 0, 0, 196),
            cursor_color: Color::new(0, 0, 0, 196),
            cursor_text_color: Color::new(255, 255, 255, 255),
        }
    }
}

impl CfgUiThemeTextview {
    fn from_yaml(yaml: &Yaml) -> CfgUiThemeTextview {
        let bgcol = yaml["background_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(255, 255, 255, 255));
        let fgcol = yaml["foreground_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(0, 0, 0, 255));
        CfgUiThemeTextview {
            background_color: bgcol,
            foreground_color: fgcol,
            cursor_color: yaml["cursor_color"]
                .as_str()
                .and_then(|s| Color::parse(s))
                .unwrap_or(fgcol),
            cursor_text_color: yaml["cursor_text_color"]
                .as_str()
                .and_then(|s| Color::parse(s))
                .unwrap_or(bgcol),
        }
    }
}

#[derive(Debug)]
pub(crate) struct CfgUiThemeGutter {
    pub(crate) background_color: Color,
    pub(crate) foreground_color: Color,
}

impl Default for CfgUiThemeGutter {
    fn default() -> CfgUiThemeGutter {
        CfgUiThemeGutter {
            background_color: Color::new(255, 255, 255, 64),
            foreground_color: Color::new(0, 0, 0, 128),
        }
    }
}

impl CfgUiThemeGutter {
    fn from_yaml(yaml: &Yaml) -> CfgUiThemeGutter {
        let bgcol = yaml["background_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(255, 255, 255, 255));
        let fgcol = yaml["foreground_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(0, 0, 0, 255));
        CfgUiThemeGutter {
            background_color: bgcol,
            foreground_color: fgcol,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CfgUiThemeFuzzy {
    pub(crate) background_color: Color,
    pub(crate) foreground_color: Color,
    pub(crate) label_color: Color,
    pub(crate) match_color: Color,
    pub(crate) select_color: Color,
    pub(crate) select_match_color: Color,
    pub(crate) select_background_color: Color,
    pub(crate) cursor_color: Color,
    pub(crate) edge_padding: u32,
}

impl Default for CfgUiThemeFuzzy {
    fn default() -> CfgUiThemeFuzzy {
        CfgUiThemeFuzzy {
            background_color: Color::new(255, 255, 255, 255),
            foreground_color: Color::new(0, 0, 0, 96),
            label_color: Color::new(0, 0, 0, 196),
            match_color: Color::new(255, 0, 0, 196),
            select_color: Color::new(0, 0, 0, 196),
            select_match_color: Color::new(255, 0, 0, 196),
            select_background_color: Color::new(0, 0, 0, 32),
            cursor_color: Color::new(255, 255, 255, 255),
            edge_padding: 10,
        }
    }
}

impl CfgUiThemeFuzzy {
    fn from_yaml(yaml: &Yaml) -> CfgUiThemeFuzzy {
        let bgcol = yaml["background_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(255, 255, 255, 255));
        let fgcol = yaml["foreground_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(0, 0, 0, 255));
        let labelcol = yaml["label_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(255, 255, 255, 255));
        let matchcol = yaml["match_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(0, 0, 0, 255));
        let selectcol = yaml["select_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(255, 255, 255, 255));
        let selectmatchcol = yaml["select_match_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(0, 0, 0, 255));
        let selectbgcol = yaml["select_background_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(255, 255, 255, 255));
        let cursorcol = yaml["cursor_color"]
            .as_str()
            .and_then(|s| Color::parse(s))
            .unwrap_or(Color::new(0, 0, 0, 255));
        let edgepad = yaml["edge_padding"].as_i64().unwrap_or(10) as u32;
        CfgUiThemeFuzzy {
            background_color: bgcol,
            foreground_color: fgcol,
            label_color: labelcol,
            match_color: matchcol,
            select_color: selectcol,
            select_match_color: selectmatchcol,
            select_background_color: selectbgcol,
            cursor_color: cursorcol,
            edge_padding: edgepad,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CfgUiThemeSyntaxElem {
    pub(crate) foreground_color: Color,
    pub(crate) text_style: TextStyle,
}

impl CfgUiThemeSyntaxElem {
    fn from_yaml(yaml: &Yaml) -> Option<CfgUiThemeSyntaxElem> {
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
            .map(|fgcol| CfgUiThemeSyntaxElem {
                foreground_color: fgcol,
                text_style: TextStyle::new(weight, slant),
            })
    }

    fn new(fg_color: Color, text_style: TextStyle) -> CfgUiThemeSyntaxElem {
        CfgUiThemeSyntaxElem {
            foreground_color: fg_color,
            text_style: text_style,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct CfgUiThemeSyntax {
    pub(crate) comment: Option<CfgUiThemeSyntaxElem>,
    pub(crate) accessor: Option<CfgUiThemeSyntaxElem>,
    pub(crate) operator: Option<CfgUiThemeSyntaxElem>,
    pub(crate) separator: Option<CfgUiThemeSyntaxElem>,
    pub(crate) keyword: Option<CfgUiThemeSyntaxElem>,
    pub(crate) identifier: Option<CfgUiThemeSyntaxElem>,
    pub(crate) data_type: Option<CfgUiThemeSyntaxElem>,
    pub(crate) escaped_char: Option<CfgUiThemeSyntaxElem>,
    pub(crate) char: Option<CfgUiThemeSyntaxElem>,
    pub(crate) string: Option<CfgUiThemeSyntaxElem>,
    pub(crate) number: Option<CfgUiThemeSyntaxElem>,
    pub(crate) func_defn: Option<CfgUiThemeSyntaxElem>,
    pub(crate) func_call: Option<CfgUiThemeSyntaxElem>,
    pub(crate) entity_name: Option<CfgUiThemeSyntaxElem>,
    pub(crate) entity_tag: Option<CfgUiThemeSyntaxElem>,
    pub(crate) h1: Option<CfgUiThemeSyntaxElem>,
}

impl CfgUiThemeSyntax {
    fn from_yaml(yaml: &Yaml) -> CfgUiThemeSyntax {
        CfgUiThemeSyntax {
            comment: CfgUiThemeSyntaxElem::from_yaml(&yaml["comment"]),
            accessor: CfgUiThemeSyntaxElem::from_yaml(&yaml["accessor"]),
            operator: CfgUiThemeSyntaxElem::from_yaml(&yaml["operator"]),
            separator: CfgUiThemeSyntaxElem::from_yaml(&yaml["separator"]),
            keyword: CfgUiThemeSyntaxElem::from_yaml(&yaml["keyword"]),
            identifier: CfgUiThemeSyntaxElem::from_yaml(&yaml["identifier"]),
            data_type: CfgUiThemeSyntaxElem::from_yaml(&yaml["data_type"]),
            escaped_char: CfgUiThemeSyntaxElem::from_yaml(&yaml["escaped_char"]),
            char: CfgUiThemeSyntaxElem::from_yaml(&yaml["char"]),
            string: CfgUiThemeSyntaxElem::from_yaml(&yaml["string"]),
            number: CfgUiThemeSyntaxElem::from_yaml(&yaml["number"]),
            func_defn: CfgUiThemeSyntaxElem::from_yaml(&yaml["func_defn"]),
            func_call: CfgUiThemeSyntaxElem::from_yaml(&yaml["func_call"]),
            entity_name: CfgUiThemeSyntaxElem::from_yaml(&yaml["entity_name"]),
            entity_tag: CfgUiThemeSyntaxElem::from_yaml(&yaml["entity_tag"]),
            h1: CfgUiThemeSyntaxElem::from_yaml(&yaml["h1"]),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct CfgUiTheme {
    pub(crate) textview: CfgUiThemeTextview,
    pub(crate) gutter: CfgUiThemeGutter,
    pub(crate) fuzzy: CfgUiThemeFuzzy,
    pub(crate) syntax: CfgUiThemeSyntax,
}

impl CfgUiTheme {
    fn from_yaml(yaml: &Yaml, cfg_dir_path: &Path) -> CfgUiTheme {
        match yaml {
            Yaml::String(s) if s.trim().split_ascii_whitespace().next() == Some("include") => {
                let target = s.trim()[7..].trim_start();
                read_to_string(cfg_dir_path.join(target))
                    .ok()
                    .and_then(|data| YamlLoader::load_from_str(&data).ok())
                    .map(|docs| CfgUiTheme::from_yaml_inner(&docs[0]))
                    .unwrap_or_else(|| CfgUiTheme::from_yaml_inner(yaml))
            }
            yaml => CfgUiTheme::from_yaml_inner(yaml),
        }
    }

    fn from_yaml_inner(yaml: &Yaml) -> CfgUiTheme {
        CfgUiTheme {
            textview: CfgUiThemeTextview::from_yaml(&yaml["textview"]),
            gutter: CfgUiThemeGutter::from_yaml(&yaml["gutter"]),
            fuzzy: CfgUiThemeFuzzy::from_yaml(&yaml["fuzzy"]),
            syntax: CfgUiThemeSyntax::from_yaml(&yaml["syntax"]),
        }
    }
}

#[derive(Debug)]
pub(crate) struct CfgUi {
    pub(crate) textview: CfgUiTextview,
    pub(crate) gutter: CfgUiGutter,
    pub(crate) fuzzy: CfgUiFuzzy,
    cur_theme: String,
    themes: HashMap<String, CfgUiTheme>,
}

impl CfgUi {
    pub(crate) fn theme(&self) -> &CfgUiTheme {
        self.themes.get(&self.cur_theme).unwrap()
    }

    fn from_yaml(yaml: &Yaml, cfg_dir_path: &Path, font_core: &mut FontCore) -> CfgUi {
        let textview = CfgUiTextview::from_yaml(yaml, font_core);
        let gutter = CfgUiGutter::from_yaml(yaml, font_core);
        let fuzzy = CfgUiFuzzy::from_yaml(yaml, font_core);
        let mut cur_theme = yaml["theme"].as_str().unwrap_or("default").to_owned();
        let mut themes = HashMap::new();
        themes.insert("default".to_owned(), CfgUiTheme::default());
        match &yaml["themes"] {
            Yaml::Hash(h) => {
                for (k, v) in h.iter() {
                    if let Some(name) = k.as_str() {
                        themes.insert(name.to_owned(), CfgUiTheme::from_yaml(v, cfg_dir_path));
                    }
                }
                if !themes.contains_key(&cur_theme) {
                    cur_theme = "default".to_owned();
                }
            }
            _ => {}
        }
        CfgUi {
            textview: textview,
            gutter: gutter,
            fuzzy: fuzzy,
            cur_theme: cur_theme,
            themes: themes,
        }
    }

    fn default(font_core: &mut FontCore) -> CfgUi {
        let default_theme = CfgUiTheme::default();
        let mut themes = HashMap::new();
        themes.insert("default".to_owned(), default_theme);
        CfgUi {
            textview: CfgUiTextview::default(font_core),
            gutter: CfgUiGutter::default(font_core),
            fuzzy: CfgUiFuzzy::default(font_core),
            cur_theme: "default".to_owned(),
            themes: themes,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CfgSyntax {
    pub(crate) tab_width: u32,
    pub(crate) indent_tabs: bool,
}

impl Default for CfgSyntax {
    fn default() -> CfgSyntax {
        CfgSyntax {
            tab_width: 8,
            indent_tabs: true,
        }
    }
}

impl CfgSyntax {
    fn from_yaml(yaml: &Yaml) -> CfgSyntax {
        CfgSyntax {
            tab_width: yaml["tab_width"].as_i64().unwrap_or(8) as u32,
            indent_tabs: yaml["indent_tabs"].as_bool().unwrap_or(true),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Cfg {
    pub(crate) ui: CfgUi,
    pub(crate) syntax: CfgSyntax,
}

impl Cfg {
    pub(crate) fn load(font_core: &mut FontCore) -> Cfg {
        if let Some(proj_dirs) = ProjectDirs::from("", "sbarua", "bed") {
            // Try loading config
            let cfg_dir_path = proj_dirs.config_dir();
            read_to_string(cfg_dir_path.join("config.yml"))
                .ok()
                .and_then(|data| YamlLoader::load_from_str(&data).ok())
                .map(|docs| Cfg::from_yaml(&docs[0], cfg_dir_path, font_core))
                .unwrap_or_else(|| Cfg::default(font_core))
        } else {
            Cfg::default(font_core)
        }
    }

    fn from_yaml(yaml: &Yaml, cfg_dir_path: &Path, font_core: &mut FontCore) -> Cfg {
        Cfg {
            ui: CfgUi::from_yaml(&yaml["ui"], cfg_dir_path, font_core),
            syntax: CfgSyntax::from_yaml(&yaml["syntax"]),
        }
    }

    fn default(font_core: &mut FontCore) -> Cfg {
        Cfg {
            ui: CfgUi::default(font_core),
            syntax: CfgSyntax::default(),
        }
    }
}

fn face_from_str(s: &str, font_core: &mut FontCore) -> Option<FaceKey> {
    s.split(',').filter_map(|s| font_core.find(s.trim())).next()
}
