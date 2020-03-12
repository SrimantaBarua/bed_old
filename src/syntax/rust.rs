// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use euclid::Size2D;
use ropey::RopeSlice;
use std::fmt::Write as FmtWrite;

use crate::config::CfgTheme;
use crate::font::FontCore;
use crate::types::{Color, TextPitch, TextSlant, TextStyle, TextWeight, DPI};
use crate::ui::text::{ShapedTextLine, TextLine, TextSpan};

use super::expand_line;

pub(crate) struct RustSyntax {}

impl RustSyntax {
    pub(super) fn new() -> RustSyntax {
        RustSyntax {}
    }

    pub(super) fn format_lines(
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
            let mut fmtline = TextLine::default();
            for tok in Lexer::new(&fmtbuf) {
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
        TokTyp::Keyword => {
            if let Some(elem) = &theme.syntax.keyword {
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

#[derive(Debug)]
enum TokTyp {
    Num,
    Comment,
    Keyword,
    Misc,
}

struct Lexer<'a> {
    s: &'a str,
}

impl<'a> Lexer<'a> {
    fn new(s: &'a str) -> Lexer<'a> {
        Lexer { s: s }
    }

    fn comment(&mut self, i: usize) -> Tok<'a> {
        let ret = &self.s[..i];
        self.s = &self.s[i..];
        Tok {
            typ: TokTyp::Comment,
            s: ret,
        }
    }

    fn num(&mut self, i: usize) -> Tok<'a> {
        let ret = &self.s[..i];
        self.s = &self.s[i..];
        Tok {
            typ: TokTyp::Num,
            s: ret,
        }
    }

    fn misc_or_key(&mut self, i: usize) -> Tok<'a> {
        let ret = &self.s[..i];
        self.s = &self.s[i..];
        Tok {
            typ: if is_key(ret) {
                TokTyp::Keyword
            } else {
                TokTyp::Misc
            },
            s: ret,
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Tok<'a>;

    fn next(&mut self) -> Option<Tok<'a>> {
        let mut iter = self.s.char_indices().peekable();
        match iter.next() {
            Some((_, '/')) => match iter.next() {
                Some((_, '/')) => Some(self.comment(self.s.len())),
                _ => Some(self.misc_or_key(1)),
            },
            Some((_, '0')) => {
                let base = match iter.next() {
                    Some((_, 'b')) | Some((_, 'B')) => 2,
                    Some((_, 'o')) | Some((_, 'O')) => 8,
                    Some((_, 'x')) | Some((_, 'X')) => 16,
                    _ => return Some(self.num(1)),
                };
                match iter.next() {
                    Some((_, c)) if c.is_digit(base) => {}
                    _ => return Some(self.num(1)),
                }
                while let Some((i, c)) = iter.next() {
                    if !c.is_digit(base) {
                        return Some(self.num(i));
                    }
                }
                Some(self.num(self.s.len()))
            }
            Some((_, c)) if c.is_digit(10) => {
                while let Some((i, c)) = iter.next() {
                    if !c.is_digit(10) {
                        return Some(self.num(i));
                    }
                }
                Some(self.num(self.s.len()))
            }
            Some((_, oc)) => {
                while let Some((i, c)) = iter.next() {
                    if c.is_digit(10) || c == '/' || (c.is_whitespace() ^ oc.is_whitespace()) {
                        return Some(self.misc_or_key(i));
                    }
                }
                Some(self.misc_or_key(self.s.len()))
            }
            None => None,
        }
    }
}

fn is_key(s: &str) -> bool {
    match s {
        "match" | "if" | "else" | "for" | "loop" | "while" | "type" | "struct" | "enum"
        | "union" | "as" | "break" | "box" | "continue" | "extern" | "fn" | "impl" | "in"
        | "let" | "pub" | "return" | "super" | "unsafe" | "where" | "use" | "mod" | "trait"
        | "move" | "mut" | "ref" | "static" | "const" | "crate" => true,
        _ => false,
    }
}
