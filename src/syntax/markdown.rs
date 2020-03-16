// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::ops::Range;

use super::{SyntaxBackend, Tok};

pub(crate) struct MarkdownSyntax {}

impl MarkdownSyntax {
    pub(super) fn new() -> MarkdownSyntax {
        MarkdownSyntax {}
    }
}

impl SyntaxBackend for MarkdownSyntax {
    fn start_of_line(&mut self, _linum: usize) {}

    fn can_end_highlight(&self) -> bool {
        true
    }

    fn insert_lines(&mut self, linum: usize, nlines: usize) {}

    fn remove_lines(&mut self, range: Range<usize>) {}

    fn next_tok<'a>(&mut self, s: &'a str) -> Option<Tok<'a>> {
        if s.len() == 0 {
            None
        } else {
            Some(Tok::misc(s).variable_pitch())
        }
    }
}
