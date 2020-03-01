// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::fs::File;
use std::io::Result as IOResult;
use std::rc::{Rc, Weak};

use ropey::{
    iter::{Chunks, Lines},
    str_utils::byte_to_char_idx,
    Rope, RopeSlice,
};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use crate::types::{Color, TextPitch, TextSize, TextSlant, TextStyle, TextWeight};
use crate::ui::text::{TextLine, TextSpan};

/// A cursor into the buffer. The buffer maintains references to all cursors, so they are
/// updated on editing the buffer
pub(crate) struct BufferCursor {
    inner: Rc<RefCell<BufferCursorInner>>,
}

struct BufferCursorWeak {
    inner: Weak<RefCell<BufferCursorInner>>,
}

struct BufferCursorInner {
    char_idx: usize,
    line_num: usize,
    line_off: usize,
}

/// A location within a buffer. This is invalidated on editing the buffer
pub(crate) struct BufferPos {
    char_idx: usize,
    line_num: usize,
    line_off: usize,
}

impl BufferPos {
    pub(crate) fn line_num(&self) -> usize {
        self.line_num
    }
}

// Actual text storage
pub(crate) struct Buffer {
    data: Rope,
    cursors: Vec<BufferCursorWeak>,
}

impl Buffer {
    /// Create empty text buffer
    pub(crate) fn empty() -> Buffer {
        Buffer {
            data: Rope::new(),
            cursors: Vec::new(),
        }
    }

    /// Create buffer from file
    pub(crate) fn from_file(path: &str) -> IOResult<Buffer> {
        File::open(path)
            .and_then(|f| Rope::from_reader(f))
            .map(|r| Buffer {
                data: r,
                cursors: Vec::new(),
            })
    }

    /// Number of lines in buffer
    pub(crate) fn len_lines(&self) -> usize {
        self.data.len_lines()
    }

    /// Get position indicator at start of line number
    pub(crate) fn get_pos_at_line(&self, linum: usize) -> BufferPos {
        if linum >= self.data.len_lines() {
            let cidx = self.data.len_chars();
            let linum = self.data.char_to_line(cidx);
            let linoff = cidx - self.data.line_to_char(linum);
            BufferPos {
                char_idx: cidx,
                line_num: linum,
                line_off: linoff,
            }
        } else {
            BufferPos {
                char_idx: self.data.line_to_char(linum),
                line_num: linum,
                line_off: 0,
            }
        }
    }

    /// Get formatted lines from point
    pub(crate) fn fmt_lines_from_pos(&self, pos: &BufferPos) -> BufferFmtLineIter {
        BufferFmtLineIter {
            lines: self.data.lines_at(pos.line_num),
        }
    }

    /// Add cursor at position
    pub(crate) fn add_cursor_at_pos(&mut self, pos: &BufferPos) -> BufferCursor {
        self.clean_cursors();
        let idx = self.cursors.binary_search_by_key(&pos.char_idx, |weak| {
            (&*weak.inner.upgrade().unwrap().borrow()).char_idx
        });
        match idx {
            Ok(idx) => BufferCursor {
                inner: self.cursors[idx].inner.upgrade().unwrap(),
            },
            Err(idx) => {
                let ret = BufferCursor {
                    inner: Rc::new(RefCell::new(BufferCursorInner {
                        char_idx: pos.char_idx,
                        line_num: pos.line_num,
                        line_off: pos.line_off,
                    })),
                };
                self.cursors.insert(
                    idx,
                    BufferCursorWeak {
                        inner: Rc::downgrade(&ret.inner),
                    },
                );
                ret
            }
        }
    }

    /// Insert character at given cursor position
    pub(crate) fn insert_char(&mut self, cursor: &mut BufferCursor, c: char) {
        // Insert character
        let char_idx = {
            let c_inner = &*cursor.inner.borrow();
            self.data.insert_char(c_inner.char_idx, c);
            c_inner.char_idx
        };

        // Update cursors after current cursor position
        self.clean_cursors();
        let idx = self.cursors.binary_search_by_key(&char_idx, |weak| {
            (&*weak.inner.upgrade().unwrap().borrow()).char_idx
        });
        let idx = match idx {
            Ok(idx) => {
                let strong = self.cursors[idx].inner.upgrade().unwrap();
                let inner = &mut *strong.borrow_mut();
                inner.char_idx += 1;
                let slice = self.data.slice(..);
                if !is_grapheme_boundary(&slice, inner.char_idx) {
                    inner.char_idx = next_grapheme_boundary(&slice, inner.char_idx);
                }
                inner.line_num = self.data.char_to_line(inner.char_idx);
                inner.line_off = inner.char_idx - self.data.line_to_char(inner.line_num);
                idx
            }
            Err(_) => panic!("cursor not found in buffer"),
        };
        for i in (idx + 1)..self.cursors.len() {
            let strong = self.cursors[i].inner.upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            inner.char_idx += 1;
            inner.line_num = self.data.char_to_line(inner.char_idx);
            inner.line_off = inner.char_idx - self.data.line_to_char(inner.line_num);
        }
    }

    /// Insert string at given cursor position
    pub(crate) fn insert_str(&mut self, cursor: &mut BufferCursor, s: &str) {
        // Get char count
        let ccount = s.chars().count();

        // Insert character
        let char_idx = {
            let c_inner = &*cursor.inner.borrow();
            self.data.insert(c_inner.char_idx, s);
            c_inner.char_idx
        };

        // Update cursors after current cursor position
        self.clean_cursors();
        let idx = self.cursors.binary_search_by_key(&char_idx, |weak| {
            (&*weak.inner.upgrade().unwrap().borrow()).char_idx
        });
        let idx = match idx {
            Ok(idx) => {
                let strong = self.cursors[idx].inner.upgrade().unwrap();
                let inner = &mut *strong.borrow_mut();
                inner.char_idx += ccount;
                let slice = self.data.slice(..);
                if !is_grapheme_boundary(&slice, inner.char_idx) {
                    inner.char_idx = next_grapheme_boundary(&slice, inner.char_idx);
                }
                inner.line_num = self.data.char_to_line(inner.char_idx);
                inner.line_off = inner.char_idx - self.data.line_to_char(inner.line_num);
                idx
            }
            Err(_) => panic!("cursor not found in buffer"),
        };
        for i in (idx + 1)..self.cursors.len() {
            let strong = self.cursors[i].inner.upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            inner.char_idx += ccount;
            inner.line_num = self.data.char_to_line(inner.char_idx);
            inner.line_off = inner.char_idx - self.data.line_to_char(inner.line_num);
        }
    }

    // TODO: Evaluate if we should do this on demand only
    fn clean_cursors(&mut self) {
        self.cursors.retain(|weak| weak.inner.strong_count() > 0);
    }
}

// From https://github.com/cessen/ropey/blob/master/examples/graphemes_step.rs
fn prev_grapheme_boundary(slice: &RopeSlice, char_idx: usize) -> usize {
    // We work with bytes for this, so convert.
    let byte_idx = slice.char_to_byte(char_idx);
    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);
    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);
    // Find the previous grapheme cluster boundary.
    loop {
        match gc.prev_boundary(chunk, chunk_byte_idx) {
            Ok(None) => return 0,
            Ok(Some(n)) => {
                let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
                return chunk_char_idx + tmp;
            }
            Err(GraphemeIncomplete::PrevChunk) => {
                let (a, b, c, _) = slice.chunk_at_byte(chunk_byte_idx - 1);
                chunk = a;
                chunk_byte_idx = b;
                chunk_char_idx = c;
            }
            Err(GraphemeIncomplete::PreContext(n)) => {
                let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                gc.provide_context(ctx_chunk, n - ctx_chunk.len());
            }
            _ => unreachable!(),
        }
    }
}

// From https://github.com/cessen/ropey/blob/master/examples/graphemes_step.rs
fn next_grapheme_boundary(slice: &RopeSlice, char_idx: usize) -> usize {
    // We work with bytes for this, so convert.
    let byte_idx = slice.char_to_byte(char_idx);
    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);
    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);
    // Find the next grapheme cluster boundary.
    loop {
        match gc.next_boundary(chunk, chunk_byte_idx) {
            Ok(None) => return slice.len_chars(),
            Ok(Some(n)) => {
                let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
                return chunk_char_idx + tmp;
            }
            Err(GraphemeIncomplete::NextChunk) => {
                chunk_byte_idx += chunk.len();
                let (a, _, c, _) = slice.chunk_at_byte(chunk_byte_idx);
                chunk = a;
                chunk_char_idx = c;
            }
            Err(GraphemeIncomplete::PreContext(n)) => {
                let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                gc.provide_context(ctx_chunk, n - ctx_chunk.len());
            }
            _ => unreachable!(),
        }
    }
}

// From https://github.com/cessen/ropey/blob/master/examples/graphemes_step.rs
fn is_grapheme_boundary(slice: &RopeSlice, char_idx: usize) -> bool {
    // We work with bytes for this, so convert.
    let byte_idx = slice.char_to_byte(char_idx);
    // Get the chunk with our byte index in it.
    let (chunk, chunk_byte_idx, _, _) = slice.chunk_at_byte(byte_idx);
    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);
    // Determine if the given position is a grapheme cluster boundary.
    loop {
        match gc.is_boundary(chunk, chunk_byte_idx) {
            Ok(n) => return n,
            Err(GraphemeIncomplete::PreContext(n)) => {
                let (ctx_chunk, ctx_byte_start, _, _) = slice.chunk_at_byte(n - 1);
                gc.provide_context(ctx_chunk, ctx_byte_start);
            }
            _ => unreachable!(),
        }
    }
}

pub(crate) struct BufferFmtLineIter<'a> {
    lines: Lines<'a>,
}

impl<'a> BufferFmtLineIter<'a> {
    pub(crate) fn prev(&mut self) -> Option<TextLine<'a>> {
        self.lines.prev().map(|l| {
            TextLine(vec![TextSpan::new(
                trim_newlines(l),
                TextSize::from_f32(8.0),
                TextStyle::new(TextWeight::Medium, TextSlant::Roman),
                Color::new(0, 0, 0, 255),
                TextPitch::Fixed,
                None,
            )])
        })
    }
}

impl<'a> Iterator for BufferFmtLineIter<'a> {
    type Item = TextLine<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.lines.next().map(|l| {
            TextLine(vec![TextSpan::new(
                trim_newlines(l),
                TextSize::from_f32(8.0),
                TextStyle::new(TextWeight::Medium, TextSlant::Roman),
                Color::new(0, 0, 0, 255),
                TextPitch::Fixed,
                None,
            )])
        })
    }
}

fn trim_newlines(slice: RopeSlice) -> RopeSlice {
    let mut end = slice.len_chars();
    let mut chars = slice.chars_at(slice.len_chars());
    while let Some(c) = chars.prev() {
        match c {
            '\n' | '\x0b' | '\x0c' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}' => end -= 1,
            _ => break,
        }
    }
    slice.slice(..end)
}

// From https://github.com/cessen/ropey/blob/master/examples/graphemes_iter.rs
pub(crate) struct RopeGraphemes<'a> {
    text: RopeSlice<'a>,
    chunks: Chunks<'a>,
    cur_chunk: &'a str,
    cur_chunk_start: usize,
    cursor: GraphemeCursor,
}

impl<'a> RopeGraphemes<'a> {
    pub(crate) fn new<'b>(slice: &RopeSlice<'b>) -> RopeGraphemes<'b> {
        let mut chunks = slice.chunks();
        let first_chunk = chunks.next().unwrap_or("");
        RopeGraphemes {
            text: *slice,
            chunks: chunks,
            cur_chunk: first_chunk,
            cur_chunk_start: 0,
            cursor: GraphemeCursor::new(0, slice.len_bytes(), true),
        }
    }
}

impl<'a> Iterator for RopeGraphemes<'a> {
    type Item = RopeSlice<'a>;

    fn next(&mut self) -> Option<RopeSlice<'a>> {
        let a = self.cursor.cur_cursor();
        let b;
        loop {
            match self
                .cursor
                .next_boundary(self.cur_chunk, self.cur_chunk_start)
            {
                Ok(None) => {
                    return None;
                }
                Ok(Some(n)) => {
                    b = n;
                    break;
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    self.cur_chunk_start += self.cur_chunk.len();
                    self.cur_chunk = self.chunks.next().unwrap_or("");
                }
                _ => unreachable!(),
            }
        }

        if a < self.cur_chunk_start {
            let a_char = self.text.byte_to_char(a);
            let b_char = self.text.byte_to_char(b);

            Some(self.text.slice(a_char..b_char))
        } else {
            let a2 = a - self.cur_chunk_start;
            let b2 = b - self.cur_chunk_start;
            Some((&self.cur_chunk[a2..b2]).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer() {
        let mut buffer = Buffer::empty();
        let pos0 = buffer.get_pos_at_line(0);
        assert_eq!(pos0.char_idx, 0);
        assert_eq!(pos0.line_num, 0);
        assert_eq!(pos0.line_off, 0);

        let mut cursor = buffer.add_cursor_at_pos(&pos0);
        buffer.insert_char(&mut cursor, 'h');
        buffer.insert_char(&mut cursor, 'e');
        buffer.insert_char(&mut cursor, 'l');
        buffer.insert_char(&mut cursor, 'l');
        buffer.insert_char(&mut cursor, 'o');

        buffer.insert_str(&mut cursor, " world");

        assert_eq!(&format!("{}", buffer.data), "hello world");
    }
}
