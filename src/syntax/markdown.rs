// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use super::{SyntaxBackend, Tok};

pub(crate) struct MarkdownSyntax {}

impl MarkdownSyntax {
    pub(super) fn new() -> MarkdownSyntax {
        MarkdownSyntax {}
    }
}

impl SyntaxBackend for MarkdownSyntax {
    fn start_of_line(&mut self) {}

    fn next_tok<'a>(&mut self, s: &'a str) -> Option<Tok<'a>> {
        if s.len() == 0 {
            None
        } else {
            Some(Tok::misc(s).variable_pitch())
        }
    }
}
