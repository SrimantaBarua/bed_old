// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::ops::Range;

use super::{SyntaxBackend, Tok};

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Base,
    BlockComment,
}

pub(crate) struct CSyntax {
    states: Vec<(State, State)>, // start, end state
    linum: usize,
}

impl CSyntax {
    pub(super) fn new() -> CSyntax {
        CSyntax {
            states: Vec::new(),
            linum: 0,
        }
    }
}

impl SyntaxBackend for CSyntax {
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
    }

    fn can_end_highlight(&self) -> bool {
        if self.linum + 1 < self.states.len() {
            self.states[self.linum].1 == self.states[self.linum + 1].0
        } else {
            true
        }
    }

    fn insert_lines(&mut self, linum: usize, nlines: usize) {
        for _ in 0..nlines {
            self.states.insert(linum, (State::Base, State::Base));
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
                (CTok::BlockCommentStart, mut i) => loop {
                    match lex.next() {
                        Some((CTok::BlockCommentEnd, j)) => {
                            break Some(Tok::comment(&s[..(i + j)]))
                        }
                        Some((_, j)) => i += j,
                        None => {
                            self.states[self.linum].1 = State::BlockComment;
                            break Some(Tok::comment(s));
                        }
                    }
                },
                (CTok::OpHash, mut i) => loop {
                    match lex.next() {
                        Some((CTok::Space, j)) => i += j,
                        Some((CTok::KeyIf, j))
                        | Some((CTok::KeyIfdef, j))
                        | Some((CTok::KeyIfndef, j))
                        | Some((CTok::KeyElif, j))
                        | Some((CTok::KeyElse, j))
                        | Some((CTok::KeyEndif, j))
                        | Some((CTok::KeyInclude, j))
                        | Some((CTok::KeyDefine, j))
                        | Some((CTok::KeyUndef, j))
                        | Some((CTok::KeyLine, j))
                        | Some((CTok::KeyError, j))
                        | Some((CTok::KeyPragma, j)) => break Some(Tok::keyword(&s[..(i + j)])),
                        x => break Some(Tok::misc(&s[..1])),
                    }
                },
                (CTok::Num, i) => Some(Tok::num(&s[..i])),
                (CTok::Keyword, i) => Some(Tok::keyword(&s[..i])),
                (CTok::Identifier, i) => Some(Tok::ident(&s[..i])),
                (CTok::Separator, i) => Some(Tok::separator(&s[..i])),
                (CTok::CommentStart, _) => Some(Tok::comment(s)),
                (CTok::Op, i) => Some(Tok::operator(&s[..i])),
                (_, i) => Some(Tok::misc(&s[..i])),
            },
            State::BlockComment => {
                let mut i = 0;
                loop {
                    match lex.next() {
                        Some((CTok::BlockCommentEnd, j)) => {
                            self.states[self.linum].1 = State::Base;
                            break Some(Tok::comment(&s[..(i + j)]));
                        }
                        Some((_, j)) => i += j,
                        None => break Some(Tok::comment(s)),
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
enum CTok {
    CommentStart,
    BlockCommentStart,
    BlockCommentEnd,
    Identifier,
    Keyword,
    KeyIf,
    KeyIfdef,
    KeyIfndef,
    KeyElif,
    KeyElse,
    KeyEndif,
    KeyInclude,
    KeyDefine,
    KeyUndef,
    KeyLine,
    KeyError,
    KeyPragma,
    Num,
    OpHash,
    Op,
    Separator,
    Accessor,
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

    fn next(&mut self) -> Option<(CTok, usize)> {
        let mut iter = self.s.char_indices().peekable();
        let (_, c1) = iter.next()?;
        let (typ, i) = match c1 {
            '#' => (CTok::OpHash, 1),
            '.' => {
                if self.s[1..].starts_with("..") {
                    (CTok::Op, 3)
                } else {
                    (CTok::Accessor, 1)
                }
            }
            ':' | '?' | '~' => (CTok::Op, 1),
            ',' | ';' => (CTok::Separator, 1),
            '/' => match iter.next() {
                Some((_, '*')) => (CTok::BlockCommentStart, 2),
                Some((_, '/')) => (CTok::CommentStart, 2),
                Some((_, '=')) => (CTok::Op, 2),
                _ => (CTok::Op, 1),
            },
            '*' => match iter.next() {
                Some((_, '/')) => (CTok::BlockCommentEnd, 2),
                Some((_, '=')) => (CTok::Op, 2),
                _ => (CTok::Op, 1),
            },
            '-' => match iter.next() {
                Some((_, '-')) | Some((_, '=')) => (CTok::Op, 2),
                Some((_, '>')) => (CTok::Accessor, 2),
                _ => (CTok::Op, 1),
            },
            '+' | '&' | '|' => match iter.next() {
                Some((_, '=')) => (CTok::Op, 2),
                Some((_, c2)) if c1 == c2 => (CTok::Op, 2),
                _ => (CTok::Op, 1),
            },
            '%' | '^' | '!' | '=' => match iter.next() {
                Some((_, '=')) => (CTok::Op, 2),
                _ => (CTok::Op, 1),
            },
            '>' | '<' => match iter.next() {
                Some((_, '=')) => (CTok::Op, 2),
                Some((_, c2)) if c1 == c2 => match iter.next() {
                    Some((_, '=')) => (CTok::Op, 3),
                    _ => (CTok::Op, 2),
                },
                _ => (CTok::Op, 1),
            },
            '0' => (CTok::Num, bin_num_or_float(self.s)),
            c if c.is_digit(10) => (CTok::Num, dec_num_or_float(self.s)),
            c if c.is_whitespace() => loop {
                if let Some((i, c)) = iter.next() {
                    if !c.is_whitespace() {
                        break (CTok::Space, i);
                    }
                } else {
                    break (CTok::Space, self.s.len());
                }
            },
            c if c == '_' || c.is_alphabetic() => loop {
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
                    (CTok::Misc, i)
                } else {
                    (CTok::Misc, self.s.len())
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
                while len < bytes.len() && bytes[len].is_ascii_digit() && bytes[len] < b'8' {
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

fn key_or_ident(s: &str) -> CTok {
    match s {
        "break" | "case" | "const" | "continue" | "default" | "do" | "enum" | "extern" | "for"
        | "goto" | "inline" | "register" | "restrict" | "return" | "sizeof" | "static"
        | "struct" | "switch" | "typedef" | "union" | "volatile" | "while" | "_Alignas"
        | "_Alignof" | "_Atomic" | "_Bool" | "_Complex" | "_Generic" | "_Imaginary"
        | "_Noreturn" | "_Static_assert" | "_Thread_local" => CTok::Keyword,
        "define" => CTok::KeyDefine,
        "elif" => CTok::KeyElif,
        "else" => CTok::KeyElse,
        "endif" => CTok::KeyEndif,
        "error" => CTok::KeyError,
        "if" => CTok::KeyIf,
        "ifdef" => CTok::KeyIfdef,
        "ifndef" => CTok::KeyIfndef,
        "include" => CTok::KeyInclude,
        "line" => CTok::KeyLine,
        "pragma" => CTok::KeyPragma,
        _ => CTok::Identifier,
    }
}
