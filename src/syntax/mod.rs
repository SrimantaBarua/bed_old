// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::fmt::Write as FmtWrite;
use std::path::Path;

use euclid::Size2D;
use ropey::RopeSlice;

use crate::config::CfgTheme;
use crate::font::FontCore;
use crate::types::{TextPitch, TextSlant, TextStyle, TextWeight, DPI};
use crate::ui::text::{ShapedTextLine, TextLine, TextSpan};

mod rust;

pub(crate) enum Syntax {
    Rust(rust::RustSyntax),
    Default,
}

impl Syntax {
    pub(crate) fn from_path(path: &str) -> Syntax {
        match Path::new(path).extension().and_then(|s| s.to_str()) {
            Some("rs") => Syntax::Rust(rust::RustSyntax::new()),
            _ => Syntax::Default,
        }
    }

    pub(crate) fn format_lines(
        &mut self,
        dpi: Size2D<u32, DPI>,
        start_linum: usize,
        opt_min_end_linum: Option<usize>,
        data: RopeSlice,
        theme: &CfgTheme,
        tabsize: usize,
        shaped_text: &mut Vec<ShapedTextLine>,
        shaped_gutter: &mut Vec<ShapedTextLine>,
        font_core: &mut FontCore,
    ) {
        match self {
            Syntax::Default => self.default_format_lines(
                dpi,
                start_linum,
                opt_min_end_linum,
                data,
                theme,
                tabsize,
                shaped_text,
                shaped_gutter,
                font_core,
            ),
            Syntax::Rust(r) => r.format_lines(
                dpi,
                start_linum,
                opt_min_end_linum,
                data,
                theme,
                tabsize,
                shaped_text,
                shaped_gutter,
                font_core,
            ),
        }
    }

    fn default_format_lines(
        &mut self,
        dpi: Size2D<u32, DPI>,
        start_linum: usize,
        opt_min_end_linum: Option<usize>,
        data: RopeSlice,
        theme: &CfgTheme,
        tabsize: usize,
        shaped_text: &mut Vec<ShapedTextLine>,
        shaped_gutter: &mut Vec<ShapedTextLine>,
        font_core: &mut FontCore,
    ) {
        let mut fmtbuf = String::new();
        for i in start_linum..data.len_lines() {
            let line = data.line(i);
            expand_line(line, tabsize, &mut fmtbuf);
            let fmtline = TextLine(vec![TextSpan::new(
                &fmtbuf,
                theme.ui.textview_text_size,
                TextStyle::new(TextWeight::Medium, TextSlant::Roman),
                theme.ui.textview_foreground_color,
                TextPitch::Fixed,
                None,
            )]);
            let shaped_line = ShapedTextLine::from_textline(
                fmtline,
                theme.ui.textview_fixed_face,
                theme.ui.textview_variable_face,
                font_core,
                dpi,
            );
            if i >= shaped_text.len() {
                shaped_text.push(shaped_line);
            } else if shaped_text[i] != shaped_line {
                shaped_text[i] = shaped_line;
            } else {
                if let Some(min) = opt_min_end_linum {
                    if i < min {
                        continue;
                    }
                }
                break;
            }
        }
        for linum in shaped_gutter.len()..(shaped_text.len() + 1) {
            fmtbuf.clear();
            write!(&mut fmtbuf, "{}", linum).unwrap();
            let fmtspan = TextSpan::new(
                &fmtbuf,
                theme.ui.gutter_text_size,
                TextStyle::new(TextWeight::Medium, TextSlant::Roman),
                theme.ui.gutter_foreground_color,
                TextPitch::Fixed,
                None,
            );
            let shaped_line = ShapedTextLine::from_textstr(
                fmtspan,
                theme.ui.gutter_fixed_face,
                theme.ui.gutter_variable_face,
                font_core,
                dpi,
            );
            shaped_gutter.push(shaped_line);
        }
    }
}

fn expand_line(slice: RopeSlice, tabsize: usize, buf: &mut String) {
    buf.clear();
    let slice = trim_newlines(slice);
    if slice.len_chars() == 0 {
        buf.push(' ');
    } else {
        let mut x = 0;
        for c in slice.chars() {
            match c {
                '\t' => {
                    let next = (x / tabsize) * tabsize + tabsize;
                    while x < next {
                        x += 1;
                        buf.push(' ');
                    }
                }
                c => {
                    buf.push(c);
                    x += 1;
                }
            }
        }
    }
}

fn trim_newlines(slice: RopeSlice) -> RopeSlice {
    let mut end = slice.len_chars();
    let mut chars = slice.chars_at(slice.len_chars());
    while let Some(c) = chars.prev() {
        match c {
            '\n' | '\x0b' | '\x0c' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}' => end -= 1,
            _ => break,
        }
    }
    slice.slice(..end)
}
