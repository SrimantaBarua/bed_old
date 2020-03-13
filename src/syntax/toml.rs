// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use super::{SyntaxBackend, Tok};

enum State {
    LineStart,
    TableNameStart,
    TableArrayNameStart,
    TableArrayNameEnd,
    TableNameEnd,
    KeyEnd,
    ValStart,
    LineEnd,
}

pub(crate) struct TOMLSyntax {
    state: State,
}

impl TOMLSyntax {
    pub(super) fn new() -> TOMLSyntax {
        TOMLSyntax {
            state: State::LineStart,
        }
    }
}

impl SyntaxBackend for TOMLSyntax {
    fn start_of_line(&mut self) {
        self.state = State::LineStart;
    }

    fn next_tok<'a>(&mut self, s: &'a str) -> Option<Tok<'a>> {
        let mut lex = Lexer::new(s);
        match self.state {
            State::LineStart => match lex.next()? {
                (TOMLTok::Lbrack, i) => {
                    self.state = State::TableNameStart;
                    Some(Tok::misc(&s[..i]))
                }
                (TOMLTok::Identifier, mut i) | (TOMLTok::String, mut i) => loop {
                    match lex.next() {
                        Some((TOMLTok::Dot, _)) => match lex.next() {
                            Some((TOMLTok::Identifier, j)) | Some((TOMLTok::String, j)) => {
                                i = j;
                            }
                            _ => {
                                self.state = State::KeyEnd;
                                break Some(Tok::entity_tag(&s[..i]));
                            }
                        },
                        _ => {
                            self.state = State::KeyEnd;
                            break Some(Tok::entity_tag(&s[..i]));
                        }
                    }
                },
                _ => Some(Tok::misc(s)),
            },
            State::TableNameStart => match lex.next()? {
                (TOMLTok::Lbrack, i) => {
                    self.state = State::TableArrayNameStart;
                    Some(Tok::misc(&s[..i]))
                }
                (TOMLTok::Identifier, mut i) | (TOMLTok::String, mut i) => loop {
                    match lex.next() {
                        Some((TOMLTok::Dot, _)) => match lex.next() {
                            Some((TOMLTok::Identifier, j)) | Some((TOMLTok::String, j)) => {
                                i = j;
                            }
                            _ => {
                                self.state = State::TableNameEnd;
                                break Some(Tok::entity_name(&s[..i]));
                            }
                        },
                        _ => {
                            self.state = State::TableNameEnd;
                            break Some(Tok::entity_name(&s[..i]));
                        }
                    }
                },
                _ => {
                    self.state = State::LineEnd;
                    Some(Tok::misc(s))
                }
            },
            State::TableArrayNameStart => match lex.next()? {
                (TOMLTok::Identifier, mut i) | (TOMLTok::String, mut i) => loop {
                    match lex.next() {
                        Some((TOMLTok::Dot, _)) => match lex.next() {
                            Some((TOMLTok::Identifier, j)) | Some((TOMLTok::String, j)) => {
                                i = j;
                            }
                            _ => {
                                self.state = State::TableArrayNameEnd;
                                break Some(Tok::entity_name(&s[..i]));
                            }
                        },
                        _ => {
                            self.state = State::TableArrayNameEnd;
                            break Some(Tok::entity_name(&s[..i]));
                        }
                    }
                },
                _ => {
                    self.state = State::LineEnd;
                    Some(Tok::misc(s))
                }
            },
            State::TableArrayNameEnd => match lex.next()? {
                (TOMLTok::Rbrack, i) => {
                    self.state = State::TableNameEnd;
                    Some(Tok::misc(&s[..i]))
                }
                _ => {
                    self.state = State::LineEnd;
                    Some(Tok::misc(s))
                }
            },
            State::TableNameEnd => match lex.next()? {
                (TOMLTok::Rbrack, i) => {
                    self.state = State::LineEnd;
                    Some(Tok::misc(&s[..i]))
                }
                _ => {
                    self.state = State::LineEnd;
                    Some(Tok::misc(s))
                }
            },
            State::KeyEnd => match lex.next()? {
                (TOMLTok::Equal, i) => {
                    self.state = State::ValStart;
                    Some(Tok::misc(&s[..i]))
                }
                _ => {
                    self.state = State::LineEnd;
                    Some(Tok::misc(s))
                }
            },
            State::ValStart => match lex.next()? {
                (TOMLTok::String, i) => {
                    self.state = State::LineEnd;
                    Some(Tok::string(&s[..i]))
                }
                (TOMLTok::Number, i) => {
                    self.state = State::LineEnd;
                    Some(Tok::num(&s[..i]))
                }
                _ => {
                    self.state = State::LineEnd;
                    Some(Tok::misc(s))
                }
            },
            State::LineEnd => match lex.next()? {
                _ => Some(Tok::misc(s)),
            },
        }
    }
}

struct Lexer<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Lexer<'a> {
    fn new(s: &str) -> Lexer {
        Lexer { s: s, i: 0 }
    }

    fn next(&mut self) -> Option<(TOMLTok, usize)> {
        let mut iter = self.s[self.i..].char_indices().peekable();

        // Skip whitespace
        while let Some((_, c)) = iter.peek() {
            if !c.is_whitespace() {
                break;
            }
            iter.next();
        }

        let (typ, i) = match iter.next() {
            Some((i, '=')) => (TOMLTok::Equal, self.i + i + 1),
            Some((i, '[')) => (TOMLTok::Lbrack, self.i + i + 1),
            Some((i, ']')) => (TOMLTok::Rbrack, self.i + i + 1),
            Some((i, ',')) => (TOMLTok::Comma, self.i + i + 1),
            Some((i, '.')) => (TOMLTok::Dot, self.i + i + 1),
            Some((_, '#')) => (TOMLTok::Comment, self.s.len()),
            Some((_, '\'')) => loop {
                if let Some((i, c)) = iter.next() {
                    if c == '\'' {
                        break (TOMLTok::String, self.i + i + 1);
                    }
                } else {
                    break (TOMLTok::Invalid, self.s.len());
                }
            },
            Some((_, '"')) => {
                let mut escape = false;
                loop {
                    if let Some((i, c)) = iter.next() {
                        if escape {
                            escape = false;
                        } else if c == '\\' {
                            escape = true;
                        } else if c == '"' {
                            break (TOMLTok::String, self.i + i + 1);
                        }
                    } else {
                        break (TOMLTok::Invalid, self.s.len());
                    }
                }
            }
            Some((_, '+')) | Some((_, '-')) => match iter.next() {
                Some((_, c)) if c.is_digit(10) => {
                    let mut is_float = false;
                    let mut last_float = false;
                    loop {
                        if let Some((i, c)) = iter.next() {
                            if c == '.' {
                                if is_float {
                                    break (TOMLTok::Number, self.i + i + 1);
                                } else {
                                    is_float = true;
                                    last_float = true;
                                }
                            } else if c.is_digit(10) {
                                last_float = false;
                            } else if last_float {
                                break (TOMLTok::Invalid, self.i + i);
                            } else {
                                break (TOMLTok::Number, self.i + i);
                            }
                        } else {
                            break (TOMLTok::Number, self.s.len());
                        }
                    }
                }
                None => {
                    if self.i == self.s.len() {
                        return None;
                    }
                    (TOMLTok::White, self.s.len())
                }
                _ => (TOMLTok::Invalid, self.s.len()),
            },
            Some((_, c)) if c.is_digit(10) => {
                let mut is_float = false;
                let mut last_float = false;
                loop {
                    if let Some((i, c)) = iter.next() {
                        if c == '.' {
                            if is_float {
                                break (TOMLTok::Number, self.i + i + 1);
                            } else {
                                is_float = true;
                                last_float = true;
                            }
                        } else if c.is_digit(10) {
                            last_float = false;
                        } else if last_float {
                            break (TOMLTok::Invalid, self.i + i);
                        } else {
                            break (TOMLTok::Number, self.i + i);
                        }
                    } else {
                        break (TOMLTok::Number, self.s.len());
                    }
                }
            }
            Some((_, c)) if c == '-' || c == '_' || c.is_ascii_alphanumeric() => loop {
                if let Some((i, c)) = iter.next() {
                    if c != '-' && c != '_' && !c.is_ascii_alphanumeric() {
                        break (TOMLTok::Identifier, self.i + i);
                    }
                } else {
                    break (TOMLTok::Identifier, self.s.len());
                }
            },
            None => {
                if self.i == self.s.len() {
                    return None;
                }
                (TOMLTok::White, self.s.len())
            }
            _ => (TOMLTok::Invalid, self.s.len()),
        };
        self.i = i;
        Some((typ, i))
    }
}

enum TOMLTok {
    Equal,
    Lbrack,
    Rbrack,
    Comma,
    Dot,
    Comment,
    String,
    Identifier,
    Invalid,
    White,
    Number,
}
