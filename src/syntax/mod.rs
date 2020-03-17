// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::default::Default;
use std::fmt::Write as FmtWrite;
use std::ops::Range;
use std::path::Path;

use euclid::Size2D;
use ropey::RopeSlice;

use crate::config::{Cfg, CfgUiTheme};
use crate::font::FontCore;
use crate::types::{Color, TextPitch, TextSlant, TextStyle, TextWeight, DPI};
use crate::ui::text::{ShapedTextLine, TextLine, TextSpan};

mod c;
mod default;
mod markdown;
mod rust;
mod toml;

trait SyntaxBackend {
    fn start_of_line(&mut self, linum: usize);

    fn can_end_highlight(&self) -> bool;

    fn insert_lines(&mut self, linum: usize, nlines: usize);

    fn remove_lines(&mut self, range: Range<usize>);

    fn next_tok<'a>(&mut self, s: &'a str) -> Option<Tok<'a>>;
}

pub(crate) enum Syntax {
    C(c::CSyntax),
    Markdown(markdown::MarkdownSyntax),
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
                "c" | "h" | "cpp" | "hpp" | "cxx" => Some(Syntax::C(c::CSyntax::new())),
                "md" => Some(Syntax::Markdown(markdown::MarkdownSyntax::new())),
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
        config: &Cfg,
        tabsize: usize,
        shaped_text: &mut Vec<ShapedTextLine>,
        shaped_gutter: &mut Vec<ShapedTextLine>,
        font_core: &mut FontCore,
    ) {
        let mut fmtbuf = String::new();
        let backend = self.get_backend();
        let theme = config.ui.theme();

        for i in start_linum..data.len_lines() {
            let line = data.line(i);
            let mut j = 0;
            let mut fmtline = TextLine::default();
            backend.start_of_line(i);
            expand_line(line, tabsize, &mut fmtbuf);

            while let Some(tok) = backend.next_tok(&fmtbuf[j..]) {
                j += tok.s.len();
                let (style, color) = tok_hl(theme, tok.typ);
                let fmtspan = TextSpan::new(
                    &tok.s,
                    config.ui.textview.text_size,
                    style,
                    color,
                    tok.pitch,
                    None,
                );
                fmtline.0.push(fmtspan);
                if j == fmtbuf.len() {
                    break;
                }
            }
            let shaped_line = ShapedTextLine::from_textline(
                fmtline,
                config.ui.textview.fixed_face,
                config.ui.textview.variable_face,
                font_core,
                dpi,
            );
            if i >= shaped_text.len() {
                shaped_text.push(shaped_line);
            } else if i == start_linum || shaped_text[i] != shaped_line {
                shaped_text[i] = shaped_line;
            } else if backend.can_end_highlight() {
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
                config.ui.gutter.text_size,
                TextStyle::new(TextWeight::Medium, TextSlant::Roman),
                theme.gutter.foreground_color,
                TextPitch::Fixed,
                None,
            );
            let shaped_line = ShapedTextLine::from_textstr(
                fmtspan,
                config.ui.gutter.fixed_face,
                config.ui.gutter.variable_face,
                font_core,
                dpi,
            );
            shaped_gutter.push(shaped_line);
        }
    }

    pub(crate) fn insert_lines(&mut self, linum: usize, nlines: usize) {
        let backend = self.get_backend();
        backend.insert_lines(linum, nlines);
    }

    pub(crate) fn remove_lines(&mut self, range: Range<usize>) {
        let backend = self.get_backend();
        backend.remove_lines(range);
    }

    fn get_backend(&mut self) -> &mut dyn SyntaxBackend {
        match self {
            Syntax::C(c) => c,
            Syntax::Rust(r) => r,
            Syntax::TOML(t) => t,
            Syntax::Markdown(m) => m,
            Syntax::Default(d) => d,
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

fn tok_hl(theme: &CfgUiTheme, typ: TokTyp) -> (TextStyle, Color) {
    match typ {
        TokTyp::Num => {
            if let Some(elem) = &theme.syntax.number {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::Comment => {
            if let Some(elem) = &theme.syntax.comment {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::Operator => {
            if let Some(elem) = &theme.syntax.operator {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::Separator => {
            if let Some(elem) = &theme.syntax.separator {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::Identifier => {
            if let Some(elem) = &theme.syntax.identifier {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::FuncDefn => {
            if let Some(elem) = &theme.syntax.func_defn {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::FuncCall => {
            if let Some(elem) = &theme.syntax.func_call {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::Keyword => {
            if let Some(elem) = &theme.syntax.keyword {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::DataType => {
            if let Some(elem) = &theme.syntax.data_type {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::EscapedChar => {
            if let Some(elem) = &theme.syntax.escaped_char {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::Char => {
            if let Some(elem) = &theme.syntax.char {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::String => {
            if let Some(elem) = &theme.syntax.string {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::EntityName => {
            if let Some(elem) = &theme.syntax.entity_name {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::EntityTag => {
            if let Some(elem) = &theme.syntax.entity_tag {
                (elem.text_style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.textview.foreground_color)
            }
        }
        TokTyp::Misc => (TextStyle::default(), theme.textview.foreground_color),
    }
}

#[derive(Debug)]
struct Tok<'a> {
    typ: TokTyp,
    s: &'a str,
    pitch: TextPitch,
}

impl<'a> Tok<'a> {
    fn operator(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Operator,
            pitch: TextPitch::Fixed,
        }
    }

    fn separator(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Separator,
            pitch: TextPitch::Fixed,
        }
    }

    fn num(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Num,
            pitch: TextPitch::Fixed,
        }
    }

    fn comment(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Comment,
            pitch: TextPitch::Fixed,
        }
    }

    fn char(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Char,
            pitch: TextPitch::Fixed,
        }
    }

    fn escaped_char(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::EscapedChar,
            pitch: TextPitch::Fixed,
        }
    }

    fn string(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::String,
            pitch: TextPitch::Fixed,
        }
    }

    fn ident(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Identifier,
            pitch: TextPitch::Fixed,
        }
    }

    fn func_defn(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::FuncDefn,
            pitch: TextPitch::Fixed,
        }
    }

    fn func_call(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::FuncCall,
            pitch: TextPitch::Fixed,
        }
    }

    fn keyword(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Keyword,
            pitch: TextPitch::Fixed,
        }
    }

    fn data_type(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::DataType,
            pitch: TextPitch::Fixed,
        }
    }

    fn entity_name(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::EntityName,
            pitch: TextPitch::Fixed,
        }
    }

    fn entity_tag(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::EntityTag,
            pitch: TextPitch::Fixed,
        }
    }

    fn misc(s: &str) -> Tok {
        Tok {
            s: s,
            typ: TokTyp::Misc,
            pitch: TextPitch::Fixed,
        }
    }

    fn variable_pitch(mut self) -> Tok<'a> {
        self.pitch = TextPitch::Variable;
        self
    }
}

#[derive(Debug)]
enum TokTyp {
    Operator,
    Separator,
    Num,
    Comment,
    EscapedChar,
    Char,
    String,
    Identifier,
    Keyword,
    DataType,
    FuncDefn,
    FuncCall,
    EntityName,
    EntityTag,
    Misc,
}
