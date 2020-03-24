// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::ops::Range;

use super::{SyntaxBackend, Tok};

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Base,
    FnDef,
    BlockComment,
    EscapedChar,
    CharEnd,
    String,
}

pub(crate) struct RustSyntax {
    states: Vec<(State, State)>, // start, end state
    linum: usize,
}

impl RustSyntax {
    pub(super) fn new() -> RustSyntax {
        RustSyntax {
            states: Vec::new(),
            linum: 0,
        }
    }
}

impl SyntaxBackend for RustSyntax {
    fn start_of_line(&mut self, linum: usize) {
        self.linum = linum;
        if self.states.len() == 0 {
            self.states.push((State::Base, State::Base));
        } else if linum >= self.states.len() {
            let prev = self.states[self.states.len() - 1].1;
            self.states.push((prev, prev));
        } else if linum == 0 {
            self.states[linum] = (State::Base, State::Base);
        } else {
            self.states[linum].0 = self.states[linum - 1].1;
            self.states[linum].1 = self.states[linum].0;
        }
        match self.states[linum].0 {
            State::CharEnd | State::EscapedChar => self.states[linum] = (State::Base, State::Base),
            _ => {}
        }
    }

    fn insert_lines(&mut self, linum: usize, nlines: usize) {
        for _ in 0..nlines {
            self.states.insert(linum, (State::Base, State::Base));
        }
    }

    fn can_end_highlight(&self) -> bool {
        if self.linum + 1 < self.states.len() {
            self.states[self.linum].1 == self.states[self.linum + 1].0
        } else {
            true
        }
    }

    fn remove_lines(&mut self, range: Range<usize>) {
        self.states.drain(range);
    }

    fn next_tok<'a>(&mut self, s: &'a str) -> Option<Tok<'a>> {
        if s.len() == 0 {
            return None;
        }
        let mut lex = Lexer::new(s);
        match self.states[self.linum].1 {
            State::Base => match lex.next()? {
                (RustTok::BlockCommentStart, mut i) => loop {
                    match lex.next() {
                        Some((RustTok::BlockCommentEnd, j)) => {
                            break Some(Tok::comment(&s[..(i + j)]))
                        }
                        Some((_, j)) => i += j,
                        None => {
                            self.states[self.linum].1 = State::BlockComment;
                            break Some(Tok::comment(s));
                        }
                    }
                },
                (RustTok::OpDoubleQuote, mut i) => loop {
                    match lex.next() {
                        Some((RustTok::OpDoubleQuote, j)) => {
                            break Some(Tok::string(&s[..(i + j)]))
                        }
                        Some((RustTok::EscapedChar, _)) => {
                            self.states[self.linum].1 = State::String;
                            break Some(Tok::string(&s[..i]));
                        }
                        Some((_, j)) => i += j,
                        None => {
                            self.states[self.linum].1 = State::String;
                            break Some(Tok::string(s));
                        }
                    }
                },
                (RustTok::OpSingleQuote, _) => {
                    let mut iter = s[1..].char_indices();
                    match iter.next() {
                        Some((_, '\\')) => {
                            self.states[self.linum].1 = State::EscapedChar;
                            Some(Tok::char(&s[..1]))
                        }
                        Some((_, '\'')) => Some(Tok::misc(&s[..1])), // TODO: Error
                        Some(_) => match iter.next() {
                            Some((i, '\'')) => Some(Tok::char(&s[..(i + 2)])),
                            _ => Some(Tok::misc(&s[..1])),
                        },
                        _ => Some(Tok::misc(&s[..1])),
                    }
                }
                (RustTok::CommentStart, _) => Some(Tok::comment(s)),
                (RustTok::Num, i) => Some(Tok::num(&s[..i])),
                (RustTok::Ident, i) => match lex.next() {
                    Some((RustTok::OpLp, _)) => Some(Tok::func_call(&s[..i])),
                    _ => Some(Tok::ident(&s[..i])),
                },
                (RustTok::OpAmp, i) => match lex.next() {
                    Some((RustTok::KeyMut, j)) => Some(Tok::operator(&s[..(i + j)])),
                    _ => Some(Tok::operator(&s[..i])),
                },
                (RustTok::Op, i) => Some(Tok::operator(&s[..i])),
                (RustTok::KeyFn, i) => {
                    self.states[self.linum].1 = State::FnDef;
                    Some(Tok::keyword(&s[..i]))
                }
                (RustTok::Key, i) | (RustTok::KeyMut, i) => Some(Tok::keyword(&s[..i])),
                (RustTok::KeyTyp, i) => Some(Tok::data_type(&s[..i])),
                (RustTok::Separator, i) => Some(Tok::separator(&s[..i])),
                (RustTok::BlockCommentEnd, i)
                | (RustTok::Space, i)
                | (RustTok::Misc, i)
                | (RustTok::EscapedChar, i)
                | (RustTok::OpLp, i) => Some(Tok::misc(&s[..i])),
            },
            State::FnDef => match lex.next()? {
                (RustTok::Space, i) => Some(Tok::misc(&s[..i])),
                (RustTok::Ident, i) => {
                    self.states[self.linum].1 = State::Base;
                    Some(Tok::func_defn(&s[..i]))
                }
                (_, i) => {
                    self.states[self.linum].1 = State::Base;
                    Some(Tok::misc(&s[..i]))
                }
            },
            State::BlockComment => {
                let mut i = 0;
                loop {
                    match lex.next() {
                        Some((RustTok::BlockCommentEnd, j)) => {
                            self.states[self.linum].1 = State::Base;
                            break Some(Tok::comment(&s[..(i + j)]));
                        }
                        Some((_, j)) => i += j,
                        None => {
                            break Some(Tok::comment(s));
                        }
                    }
                }
            }
            State::CharEnd => {
                self.states[self.linum].1 = State::Base;
                if s.as_bytes()[0] == b'\'' {
                    Some(Tok::char(&s[..1]))
                } else {
                    Some(Tok::misc(&s[..1]))
                }
            }
            State::EscapedChar => {
                if let Some(l) = escaped_char(&s[1..]) {
                    self.states[self.linum].1 = State::CharEnd;
                    Some(Tok::escaped_char(&s[..(l + 1)]))
                } else {
                    self.states[self.linum].1 = State::Base;
                    Some(Tok::misc(&s[..1]))
                }
            }
            State::String => {
                let mut i = 0;
                loop {
                    match lex.next() {
                        Some((RustTok::OpDoubleQuote, j)) => {
                            self.states[self.linum].1 = State::Base;
                            break Some(Tok::string(&s[..(i + j)]));
                        }
                        Some((RustTok::EscapedChar, j)) => {
                            if i == 0 {
                                break Some(Tok::escaped_char(&s[..(i + j)]));
                            } else {
                                break Some(Tok::string(&s[..i]));
                            }
                        }
                        Some((_, j)) => i += j,
                        None => break Some(Tok::string(s)),
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
enum RustTok {
    CommentStart,
    BlockCommentStart,
    BlockCommentEnd,
    EscapedChar,
    Num,
    Ident,
    Separator,
    OpLp,
    OpAmp,
    OpDoubleQuote,
    OpSingleQuote,
    Op,
    KeyFn,
    KeyMut,
    KeyTyp,
    Key,
    Space,
    Misc,
}

struct Lexer<'a> {
    s: &'a str,
}

impl<'a> Lexer<'a> {
    fn new(s: &str) -> Lexer {
        Lexer { s: s }
    }

    fn next(&mut self) -> Option<(RustTok, usize)> {
        let mut iter = self.s.char_indices().peekable();
        let (typ, i) = match iter.next()? {
            (_, '(') => (RustTok::OpLp, 1),
            (_, ';') | (_, ',') => (RustTok::Separator, 1),
            (_, '/') => match iter.next() {
                // TODO doc comment
                Some((_, '*')) => (RustTok::BlockCommentStart, 2),
                Some((_, '/')) => (RustTok::CommentStart, 2),
                Some((_, '=')) => (RustTok::Op, 2),
                _ => (RustTok::Op, 1),
            },
            (_, '*') => match iter.next() {
                Some((_, '/')) => (RustTok::BlockCommentEnd, 2),
                Some((_, '=')) => (RustTok::Op, 2),
                _ => (RustTok::Op, 1),
            },
            (_, '+') | (_, '%') | (_, '^') | (_, '!') => match iter.next() {
                Some((_, '=')) => (RustTok::Op, 2),
                _ => (RustTok::Op, 1),
            },
            (_, '-') => match iter.next() {
                Some((_, '=')) => (RustTok::Op, 2),
                // TODO: ->
                _ => (RustTok::Op, 1),
            },
            (_, '=') => match iter.next() {
                Some((_, '=')) => (RustTok::Op, 2),
                Some((_, '>')) => (RustTok::Op, 2),
                _ => (RustTok::Op, 1),
            },
            (_, '|') => match iter.next() {
                Some((_, '|')) | Some((_, '=')) => (RustTok::Op, 2),
                _ => (RustTok::Op, 1),
            },
            (_, '&') => match iter.next() {
                Some((_, '&')) | Some((_, '=')) => (RustTok::Op, 2),
                _ => (RustTok::OpAmp, 1),
            },
            (_, '>') => match iter.next() {
                Some((_, '>')) => match iter.next() {
                    Some((_, '=')) => (RustTok::Op, 3),
                    _ => (RustTok::Op, 2),
                },
                Some((_, '=')) => (RustTok::Op, 2),
                _ => (RustTok::Op, 1),
            },
            (_, '<') => match iter.next() {
                Some((_, '<')) => match iter.next() {
                    Some((_, '=')) => (RustTok::Op, 3),
                    _ => (RustTok::Op, 2),
                },
                Some((_, '=')) => (RustTok::Op, 2),
                _ => (RustTok::Op, 1),
            },
            // TODO raw strings etc
            // TODO multi-line strings
            (_, '"') => (RustTok::OpDoubleQuote, 1),
            (_, '\'') => (RustTok::OpSingleQuote, 1),
            (_, '\\') => {
                if let Some(l) = escaped_char(&self.s[1..]) {
                    (RustTok::EscapedChar, l)
                } else {
                    (RustTok::Misc, 1)
                }
            }
            (_, '0') => (RustTok::Num, bin_num_or_float(self.s)),
            (_, c) if c.is_digit(10) => (RustTok::Num, dec_num_or_float(self.s)),
            (_, c) if c.is_whitespace() => loop {
                if let Some((i, c)) = iter.next() {
                    if !c.is_whitespace() {
                        break (RustTok::Space, i);
                    }
                } else {
                    break (RustTok::Space, self.s.len());
                }
            },
            (_, c) if c == '_' || c.is_alphabetic() => loop {
                if let Some((i, c)) = iter.next() {
                    if c != '_' && !c.is_alphanumeric() {
                        break (key_or_ident(&self.s[..i]), i);
                    }
                } else {
                    break (key_or_ident(self.s), self.s.len());
                }
            },
            _ => {
                if let Some((i, _)) = iter.next() {
                    (RustTok::Misc, i)
                } else {
                    (RustTok::Misc, self.s.len())
                }
            }
        };
        self.s = &self.s[i..];
        Some((typ, i))
    }
}

fn bin_num_or_float(s: &str) -> usize {
    let bytes = s.as_bytes();
    if bytes.len() < 3 {
        return 1;
    }
    match bytes[1] {
        b'b' | b'B' => {
            if bytes[2] == b'0' || bytes[2] == b'1' {
                let mut len = 3;
                // TODO: Re-evaluate terminating underscore highlighting
                while len < bytes.len()
                    && (bytes[len] == b'0' || bytes[len] == b'1' || bytes[len] == b'_')
                {
                    len += 1;
                }
                len
            } else {
                1
            }
        }
        b'o' | b'O' => {
            if bytes[2].is_ascii_digit() && bytes[2] < b'8' {
                let mut len = 3;
                // TODO: Re-evaluate terminating underscore highlighting
                while len < bytes.len()
                    && (bytes[len] == b'_' || (bytes[len].is_ascii_digit() && bytes[len] < b'8'))
                {
                    len += 1;
                }
                len
            } else {
                1
            }
        }
        b'x' | b'X' => {
            if bytes[2].is_ascii_hexdigit() {
                let mut len = 3;
                // TODO: Re-evaluate terminating underscore highlighting
                while len < bytes.len() && (bytes[len] == b'_' || bytes[len].is_ascii_hexdigit()) {
                    len += 1;
                }
                len
            } else {
                1
            }
        }
        b'.' => 1 + float_len_from_decimal(&s[1..]),
        _ => 1,
    }
}

fn dec_num_or_float(s: &str) -> usize {
    let mut len = 1;
    let bytes = s.as_bytes();
    // TODO: Re-evalute terminating underscore highlighting
    while len < bytes.len() && (bytes[len] == b'_' || bytes[len].is_ascii_digit()) {
        len += 1;
    }
    len + float_len_from_decimal(&s[len..])
}

fn float_len_from_decimal(s: &str) -> usize {
    let bytes = s.as_bytes();
    // TODO: Re-evaluate highlighting of underscore in floats
    if bytes.len() < 2 || bytes[0] != b'.' || !bytes[1].is_ascii_digit() {
        return 0;
    }
    let mut len = 2;
    while len < bytes.len() && (bytes[len] == b'_' || bytes[len].is_ascii_digit()) {
        len += 1;
    }
    len
}

fn escaped_char(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.len() < 1 {
        return None;
    }
    match bytes[0] {
        b'\\' | b'\'' | b'"' | b't' | b'n' | b'r' => Some(1),
        b'x' => {
            if bytes.len() < 3 {
                None
            } else {
                if bytes[1].is_ascii_digit() && bytes[1] < b'8' && bytes[2].is_ascii_hexdigit() {
                    Some(3)
                } else {
                    None
                }
            }
        }
        b'u' => {
            if bytes.len() < 4 || bytes[1] != b'{' {
                None
            } else {
                let mut len = 2;
                while len < 8 && len < bytes.len() {
                    if !bytes[len].is_ascii_hexdigit() {
                        break;
                    }
                    len += 1;
                }
                if len == 2 || len == bytes.len() || bytes[len] != b'}' {
                    None
                } else {
                    Some(len + 1)
                }
            }
        }
        _ => None,
    }
}

fn key_or_ident(s: &str) -> RustTok {
    match s {
        "abstract" | "as" | "async" | "await" | "become" | "box" | "break" | "const"
        | "continue" | "crate" | "do" | "dyn" | "else" | "enum" | "extern" | "final" | "for"
        | "if" | "impl" | "in" | "let" | "loop" | "macro" | "match" | "mod" | "move"
        | "override" | "priv" | "pub" | "ref" | "return" | "self" | "Self" | "static"
        | "struct" | "super" | "trait" | "try" | "type" | "typeof" | "union" | "unsafe"
        | "unsized" | "use" | "virtual" | "where" | "while" | "yield" => RustTok::Key,
        "fn" => RustTok::KeyFn,
        "mut" => RustTok::KeyMut,
        "bool" | "char" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16"
        | "u32" | "u64" | "u128" | "usize" | "f32" | "f64" | "str" => RustTok::KeyTyp,
        "true" | "false" => RustTok::Num,
        _ => RustTok::Ident,
    }
}
