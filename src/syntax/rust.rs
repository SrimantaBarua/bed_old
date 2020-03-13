// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use super::{SyntaxBackend, Tok};

enum State {
    Base,
    FnDef,
}

pub(crate) struct RustSyntax {
    state: State,
}

impl RustSyntax {
    pub(super) fn new() -> RustSyntax {
        RustSyntax { state: State::Base }
    }
}

impl SyntaxBackend for RustSyntax {
    fn start_of_line(&mut self) {
        self.state = State::Base;
    }

    fn next_tok<'a>(&mut self, s: &'a str) -> Option<Tok<'a>> {
        let mut lex = Lexer::new(s);
        match self.state {
            State::Base => match lex.next()? {
                (RustTok::Comment, i) => Some(Tok::comment(&s[..i])),
                (RustTok::String, i) | (RustTok::Char, i) => Some(Tok::string(&s[..i])),
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
                    self.state = State::FnDef;
                    Some(Tok::keyword(&s[..i]))
                }
                (RustTok::Key, i) | (RustTok::KeyMut, i) => Some(Tok::keyword(&s[..i])),
                (RustTok::Space, i) | (RustTok::Misc, i) | (RustTok::OpLp, i) => {
                    Some(Tok::misc(&s[..i]))
                }
            },
            State::FnDef => match lex.next()? {
                (RustTok::Space, i) => Some(Tok::misc(&s[..i])),
                (RustTok::Ident, i) => {
                    self.state = State::Base;
                    Some(Tok::func_defn(&s[..i]))
                }
                (_, i) => {
                    self.state = State::Base;
                    Some(Tok::misc(&s[..i]))
                }
            },
        }
    }
}

enum RustTok {
    Comment,
    String,
    Char,
    Num,
    Ident,
    OpLp,
    OpAmp,
    Op,
    KeyFn,
    KeyMut,
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
            (_, '&') => (RustTok::OpAmp, 1),
            (_, '(') => (RustTok::OpLp, 1),
            (_, '/') => match iter.next() {
                // TODO Block comment, doc comment
                Some((_, '/')) => (RustTok::Comment, self.s.len()),
                Some((_, '=')) => (RustTok::Op, 2),
                _ => (RustTok::Op, 1),
            },
            // TODO raw strings etc
            // TODO multi-line strings
            (_, '"') => {
                let mut escaped = false;
                loop {
                    if let Some((i, c)) = iter.next() {
                        if c == '/' {
                            escaped = !escaped;
                        } else if c == '"' && !escaped {
                            break (RustTok::String, i + 1);
                        }
                    } else {
                        break (RustTok::String, self.s.len());
                    }
                }
            }
            (_, '\'') => match iter.next() {
                Some((_, '\\')) => {
                    let mut bytes = self.s[2..].bytes();
                    match bytes.next() {
                        Some(b'\'') | Some(b'"') | Some(b't') | Some(b'n') | Some(b'r') => {
                            match bytes.next() {
                                Some(b'\'') => (RustTok::Char, 4),
                                _ => (RustTok::Misc, 1),
                            }
                        }
                        Some(b'x') => match bytes.next() {
                            Some(b) if b.is_ascii_digit() && b < b'8' => match bytes.next() {
                                Some(b) if b.is_ascii_hexdigit() => match bytes.next() {
                                    Some(b'\'') => (RustTok::Char, 6),
                                    _ => (RustTok::Misc, 1),
                                },
                                _ => (RustTok::Misc, 1),
                            },
                            _ => (RustTok::Misc, 1),
                        },
                        Some(b'u') => match bytes.next() {
                            Some(b'{') => {
                                let mut len = 0;
                                while let Some(b) = bytes.next() {
                                    if !b.is_ascii_hexdigit() {
                                        break;
                                    }
                                    len += 1;
                                    if len == 6 {
                                        break;
                                    }
                                }
                                match bytes.next() {
                                    Some(b'}') => match bytes.next() {
                                        Some(b'\'') => (RustTok::Char, len + 6),
                                        _ => (RustTok::Misc, 1),
                                    },
                                    _ => (RustTok::Misc, 1),
                                }
                            }
                            _ => (RustTok::Misc, 1),
                        },
                        _ => (RustTok::Misc, 1),
                    }
                }
                Some((_, '\'')) => (RustTok::Misc, 1), // TODO: Error
                Some(_) => match iter.next() {
                    Some((i, '\'')) => (RustTok::Char, i + 1),
                    _ => (RustTok::Misc, 1),
                },
                _ => (RustTok::Misc, 1),
            },
            (_, '0') => (RustTok::Num, bin_num_or_float(self.s)),
            (_, c) if c.is_whitespace() => loop {
                if let Some((i, c)) = iter.next() {
                    if !c.is_whitespace() {
                        break (RustTok::Space, i);
                    }
                } else {
                    break (RustTok::Space, self.s.len());
                }
            },
            (_, c) if c.is_digit(10) => (RustTok::Num, dec_num_or_float(self.s)),
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
                // TODO Remove this
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
                while len < bytes.len() && (bytes[len] == b'0' || bytes[len] == b'1') {
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
                while len < bytes.len() && (bytes[len].is_ascii_digit() && bytes[len] < b'8') {
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
                while len < bytes.len() && bytes[len].is_ascii_hexdigit() {
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
    while len < bytes.len() && bytes[len].is_ascii_digit() {
        len += 1;
    }
    len + float_len_from_decimal(&s[len..])
}

fn float_len_from_decimal(s: &str) -> usize {
    let bytes = s.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'.' || !bytes[1].is_ascii_digit() {
        return 0;
    }
    let mut len = 2;
    while len < bytes.len() && bytes[len].is_ascii_digit() {
        len += 1;
    }
    len
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
        _ => RustTok::Ident,
    }
}
