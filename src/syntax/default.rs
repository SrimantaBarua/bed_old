// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use super::{SyntaxBackend, Tok};

pub(crate) struct DefaultSyntax;

impl SyntaxBackend for DefaultSyntax {
    fn start_of_line(&mut self) {}

    fn next_tok<'a>(&mut self, s: &'a str) -> Option<Tok<'a>> {
        if s.len() == 0 {
            None
        } else {
            Some(Tok::misc(s))
        }
    }
}
