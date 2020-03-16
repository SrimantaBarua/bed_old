// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::ops::Range;

use super::{SyntaxBackend, Tok};

enum State {
    Start,
}

pub(crate) struct CSyntax {
    state: State,
}

impl CSyntax {
    pub(super) fn new() -> CSyntax {
        CSyntax {
            state: State::Start,
        }
    }
}

impl SyntaxBackend for CSyntax {
    fn start_of_line(&mut self, _linum: usize) {
        self.state = State::Start;
    }

    fn can_end_highlight(&self) -> bool {
        true
    }

    fn insert_lines(&mut self, linum: usize, nlines: usize) {}

    fn remove_lines(&mut self, range: Range<usize>) {}

    fn next_tok<'a>(&mut self, s: &'a str) -> Option<Tok<'a>> {
        let mut lex = Lexer::new(s);
        match self.state {
            State::Start => match lex.next()? {
                (CTok::Comment, i) => Some(Tok::comment(&s[..i])),
                (_, i) => Some(Tok::misc(&s[..i])),
            },
        }
    }
}

enum CTok {
    Comment,
    Misc,
    White,
}

struct Lexer<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Lexer<'a> {
    fn new(s: &str) -> Lexer {
        Lexer { s: s, i: 0 }
    }

    fn next(&mut self) -> Option<(CTok, usize)> {
        let mut iter = self.s[self.i..].char_indices().peekable();

        // Skip whitespace
        while let Some((_, c)) = iter.peek() {
            if !c.is_whitespace() {
                break;
            }
            iter.next();
        }

        let (typ, i) = match iter.next() {
            Some((_, '/')) => match iter.next() {
                Some((_, '/')) => (CTok::Comment, self.s.len()),
                _ => (CTok::Misc, self.s.len()),
            },
            Some((_, _)) => (CTok::Misc, self.s.len()),
            None => {
                if self.i == self.s.len() {
                    return None;
                }
                (CTok::White, self.s.len())
            }
        };
        self.i = i;
        Some((typ, i))
    }
}
