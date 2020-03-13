// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use super::{SyntaxBackend, Tok};

enum State {
    Start,
}

pub(crate) struct RustSyntax {
    state: State,
}

impl RustSyntax {
    pub(super) fn new() -> RustSyntax {
        RustSyntax {
            state: State::Start,
        }
    }
}

impl SyntaxBackend for RustSyntax {
    fn start_of_line(&mut self) {
        self.state = State::Start;
    }

    fn next_tok<'a>(&mut self, s: &'a str) -> Option<Tok<'a>> {
        let mut lex = Lexer::new(s);
        match self.state {
            State::Start => match lex.next()? {
                (RustTok::Comment, i) => Some(Tok::comment(&s[..i])),
                (_, i) => Some(Tok::misc(&s[..i])),
            },
        }
    }
}

enum RustTok {
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

    fn next(&mut self) -> Option<(RustTok, usize)> {
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
                Some((_, '/')) => (RustTok::Comment, self.s.len()),
                _ => (RustTok::Misc, self.s.len()),
            },
            Some((_, _)) => (RustTok::Misc, self.s.len()),
            None => {
                if self.i == self.s.len() {
                    return None;
                }
                (RustTok::White, self.s.len())
            }
        };
        self.i = i;
        Some((typ, i))
    }
}

/*
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
*/
