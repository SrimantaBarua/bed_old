// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::default::Default;
use std::fmt::Write as FmtWrite;
use std::path::Path;

use euclid::Size2D;
use ropey::RopeSlice;

use crate::config::CfgTheme;
use crate::font::FontCore;
use crate::types::{Color, TextPitch, TextSlant, TextStyle, TextWeight, DPI};
use crate::ui::text::{ShapedTextLine, TextLine, TextSpan};

mod default;
mod rust;
mod toml;

trait SyntaxBackend {
    fn start_of_line(&mut self);

    fn next_tok<'a>(&mut self, s: &'a str) -> Option<Tok<'a>>;
}

pub(crate) enum Syntax {
    Rust(rust::RustSyntax),
    TOML(toml::TOMLSyntax),
    Default(default::DefaultSyntax),
}

impl Default for Syntax {
    fn default() -> Syntax {
        Syntax::Default(default::DefaultSyntax)
    }
}

impl Syntax {
    pub(crate) fn from_path(path: &str) -> Syntax {
        Path::new(path)
            // Try with extension
            .extension()
            .and_then(|s| s.to_str())
            .and_then(|s| match s {
                "rs" => Some(Syntax::Rust(rust::RustSyntax::new())),
                "toml" => Some(Syntax::TOML(toml::TOMLSyntax::new())),
                _ => None,
            })
            // TODO: Try with filename
            .unwrap_or_default()
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
        let mut fmtbuf = String::new();
        let backend = self.get_backend();

        for i in start_linum..data.len_lines() {
            let line = data.line(i);
            let mut j = 0;
            let mut fmtline = TextLine::default();
            backend.start_of_line();
            expand_line(line, tabsize, &mut fmtbuf);

            let shaped_line = if fmtbuf.len() > 0 {
                while let Some(tok) = backend.next_tok(&fmtbuf[j..]) {
                    j += tok.s.len();
                    let (style, color) = tok_hl(theme, tok.typ);
                    let fmtspan = TextSpan::new(
                        &tok.s,
                        theme.ui.textview_text_size,
                        style,
                        color,
                        TextPitch::Fixed,
                        None,
                    );
                    fmtline.0.push(fmtspan);
                }
                ShapedTextLine::from_textline(
                    fmtline,
                    theme.ui.textview_fixed_face,
                    theme.ui.textview_variable_face,
                    font_core,
                    dpi,
                )
            } else {
                ShapedTextLine::from_textstr(
                    TextSpan::new(
                        " ",
                        theme.ui.textview_text_size,
                        TextStyle::default(),
                        theme.ui.textview_foreground_color,
                        TextPitch::Fixed,
                        None,
                    ),
                    theme.ui.textview_fixed_face,
                    theme.ui.textview_variable_face,
                    font_core,
                    dpi,
                )
            };
            if i >= shaped_text.len() {
                shaped_text.push(shaped_line);
            } else if i == start_linum || shaped_text[i] != shaped_line {
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

    fn get_backend(&mut self) -> &mut dyn SyntaxBackend {
        match self {
            Syntax::Rust(r) => r,
            Syntax::TOML(t) => t,
            Syntax::Default(d) => d,
        }
    }
}

fn expand_line(slice: RopeSlice, tabsize: usize, buf: &mut String) {
    buf.clear();
    let slice = trim_newlines(slice);
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

fn tok_hl(theme: &CfgTheme, typ: TokTyp) -> (TextStyle, Color) {
    match typ {
        TokTyp::Num => {
            if let Some(elem) = &theme.syntax.number {
                (elem.style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.ui.textview_foreground_color)
            }
        }
        TokTyp::Comment => {
            if let Some(elem) = &theme.syntax.comment {
                (elem.style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.ui.textview_foreground_color)
            }
        }
        TokTyp::Operator => {
            if let Some(elem) = &theme.syntax.operator {
                (elem.style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.ui.textview_foreground_color)
            }
        }
        TokTyp::Separator => {
            if let Some(elem) = &theme.syntax.separator {
                (elem.style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.ui.textview_foreground_color)
            }
        }
        TokTyp::Identifier => {
            if let Some(elem) = &theme.syntax.identifier {
                (elem.style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.ui.textview_foreground_color)
            }
        }
        TokTyp::Keyword => {
            if let Some(elem) = &theme.syntax.keyword {
                (elem.style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.ui.textview_foreground_color)
            }
        }
        TokTyp::String => {
            if let Some(elem) = &theme.syntax.string {
                (elem.style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.ui.textview_foreground_color)
            }
        }
        TokTyp::EntityName => {
            if let Some(elem) = &theme.syntax.entity_name {
                (elem.style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.ui.textview_foreground_color)
            }
        }
        TokTyp::EntityTag => {
            if let Some(elem) = &theme.syntax.entity_tag {
                (elem.style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.ui.textview_foreground_color)
            }
        }
        TokTyp::Misc => (TextStyle::default(), theme.ui.textview_foreground_color),
    }
}

#[derive(Debug)]
struct Tok<'a> {
    typ: TokTyp,
    s: &'a str,
}

impl<'a> Tok<'a> {
    fn operator(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Operator,
        }
    }

    fn separator(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Separator,
        }
    }

    fn num(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Num,
        }
    }

    fn comment(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Comment,
        }
    }

    fn string(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::String,
        }
    }

    fn ident(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Identifier,
        }
    }

    fn keyword(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Keyword,
        }
    }

    fn entity_name(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::EntityName,
        }
    }

    fn entity_tag(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::EntityTag,
        }
    }

    fn misc(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Misc,
        }
    }
}

#[derive(Debug)]
enum TokTyp {
    Operator,
    Separator,
    Num,
    Comment,
    String,
    Identifier,
    Keyword,
    EntityName,
    EntityTag,
    Misc,
}
