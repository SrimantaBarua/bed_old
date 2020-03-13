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
            let mut lex = Lexer::new(&fmtbuf);
            while let Some(tok) = lex.next() {
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
        TokTyp::Operator => {
            if let Some(elem) = &theme.syntax.operator {
                (elem.style, elem.foreground_color)
            } else {
                (TextStyle::default(), theme.ui.textview_foreground_color)
            }
        }
        TokTyp::Separator | TokTyp::SepSemi => {
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
        TokTyp::Keyword | TokTyp::KeyUse => {
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
    Operator,
    Separator,
    SepSemi,
    Num,
    Comment,
    String,
    Identifier,
    Misc,
    Keyword,
    KeyUse,
}

struct Lexer<'a> {
    s: &'a str,
}

impl<'a> Lexer<'a> {
    fn new(s: &'a str) -> Lexer<'a> {
        Lexer { s: s }
    }

    fn op(&mut self, i: usize) -> Tok<'a> {
        let ret = &self.s[..i];
        self.s = &self.s[i..];
        Tok {
            typ: TokTyp::Operator,
            s: ret,
        }
    }

    fn sep(&mut self, i: usize) -> Tok<'a> {
        let ret = &self.s[..i];
        self.s = &self.s[i..];
        Tok {
            typ: TokTyp::Separator,
            s: ret,
        }
    }

    fn sep_semi(&mut self, i: usize) -> Tok<'a> {
        let ret = &self.s[..i];
        self.s = &self.s[i..];
        Tok {
            typ: TokTyp::SepSemi,
            s: ret,
        }
    }

    fn string(&mut self, i: usize) -> Tok<'a> {
        let ret = &self.s[..i];
        self.s = &self.s[i..];
        Tok {
            typ: TokTyp::String,
            s: ret,
        }
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

    fn ident_or_key(&mut self, i: usize) -> Tok<'a> {
        let ret = &self.s[..i];
        self.s = &self.s[i..];
        Tok {
            typ: key_or_ident(ret),
            s: ret,
        }
    }

    fn misc(&mut self, i: usize) -> Tok<'a> {
        let ret = &self.s[..i];
        self.s = &self.s[i..];
        Tok {
            typ: TokTyp::Misc,
            s: ret,
        }
    }

    fn next(&mut self) -> Option<Tok<'a>> {
        let mut iter = self.s.char_indices().peekable();
        match iter.next() {
            Some((_, '+')) => match iter.next() {
                Some((_, '=')) => Some(self.op(2)),
                _ => Some(self.op(1)),
            },
            Some((_, '-')) => match iter.next() {
                Some((_, '=')) => Some(self.op(2)),
                _ => Some(self.op(1)),
            },
            Some((_, '*')) => match iter.next() {
                Some((_, '=')) => Some(self.op(2)),
                _ => Some(self.op(1)),
            },
            Some((_, '/')) => match iter.next() {
                Some((_, '=')) => Some(self.op(2)),
                Some((_, '/')) => Some(self.comment(self.s.len())),
                _ => Some(self.op(1)),
            },
            Some((_, '%')) | Some((_, '^')) | Some((_, '!')) => match iter.next() {
                Some((_, '=')) => Some(self.op(2)),
                _ => Some(self.op(1)),
            },
            Some((_, '?')) => Some(self.op(1)),
            Some((_, '&')) => {
                if self.s[1..].starts_with("mut") {
                    if self.s.len() == 4 {
                        return Some(self.op(4));
                    }
                    let c = self.s[4..].chars().next().unwrap();
                    if c != '_' && !c.is_digit(10) && !c.is_alphabetic() {
                        return Some(self.op(4));
                    }
                }
                match iter.next() {
                    Some((_, '&')) | Some((_, '=')) => Some(self.op(2)),
                    _ => Some(self.op(1)),
                }
            }
            Some((_, '|')) => match iter.next() {
                Some((_, '|')) | Some((_, '=')) => Some(self.op(2)),
                _ => Some(self.op(1)),
            },
            Some((_, '=')) => match iter.next() {
                Some((_, '=')) | Some((_, '>')) => Some(self.op(2)),
                _ => Some(self.op(1)),
            },
            Some((_, '<')) => match iter.next() {
                Some((_, '<')) => match iter.next() {
                    Some((_, '=')) => Some(self.op(3)),
                    _ => Some(self.op(2)),
                },
                Some((_, '=')) => Some(self.op(2)),
                _ => Some(self.op(1)),
            },
            Some((_, '>')) => match iter.next() {
                Some((_, '>')) => match iter.next() {
                    Some((_, '=')) => Some(self.op(3)),
                    _ => Some(self.op(2)),
                },
                Some((_, '=')) => Some(self.op(2)),
                _ => Some(self.op(1)),
            },
            Some((_, ':')) => match iter.next() {
                Some((_, ':')) => Some(self.op(2)),
                _ => Some(self.sep(1)),
            },
            Some((_, '.')) => match iter.next() {
                Some((_, '.')) => match iter.next() {
                    Some((_, '.')) | Some((_, '=')) => Some(self.op(3)),
                    _ => Some(self.op(2)),
                },
                _ => Some(self.misc(1)),
            },
            Some((_, ';')) => Some(self.sep_semi(1)),
            Some((_, ',')) => Some(self.sep(1)),
            Some((_, 'r')) if iter.peek() == Some(&(0, '"')) => {
                iter.next();
                while let Some((_, c)) = iter.next() {
                    if c == '"' {
                        break;
                    }
                }
                if let Some((i, _)) = iter.next() {
                    Some(self.string(i))
                } else {
                    // TODO: Strings spanning multiple lines
                    Some(self.string(self.s.len()))
                }
            }
            Some((_, '"')) => {
                while let Some((_, c)) = iter.next() {
                    if c == '"' {
                        break;
                    }
                }
                if let Some((i, _)) = iter.next() {
                    Some(self.string(i))
                } else {
                    // TODO: Strings spanning multiple lines
                    Some(self.string(self.s.len()))
                }
            }
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
            Some((_, c)) if c == '_' || c.is_alphabetic() => {
                while let Some((i, c)) = iter.next() {
                    if c != '_' && !c.is_digit(10) && !c.is_alphabetic() {
                        return Some(self.ident_or_key(i));
                    }
                }
                Some(self.ident_or_key(self.s.len()))
            }
            Some((_, oc)) => {
                while let Some((i, c)) = iter.next() {
                    if c.is_digit(10) || c == '/' || (c.is_whitespace() ^ oc.is_whitespace()) {
                        return Some(self.misc(i));
                    }
                }
                Some(self.misc(self.s.len()))
            }
            None => None,
        }
    }
}

fn key_or_ident(s: &str) -> TokTyp {
    match s {
        "abstract" | "as" | "async" | "await" | "become" | "box" | "break" | "const"
        | "continue" | "crate" | "do" | "dyn" | "else" | "enum" | "extern" | "final" | "fn"
        | "for" | "if" | "impl" | "in" | "let" | "loop" | "macro" | "match" | "mod" | "move"
        | "mut" | "override" | "priv" | "pub" | "ref" | "return" | "self" | "Self" | "static"
        | "struct" | "super" | "trait" | "try" | "type" | "typeof" | "union" | "unsafe"
        | "unsized" | "virtual" | "where" | "while" | "yield" => TokTyp::Keyword,
        "use" => TokTyp::KeyUse,
        _ => TokTyp::Identifier,
    }
}
