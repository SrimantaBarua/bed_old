// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::fs::File;
use std::io::Result as IOResult;
use std::rc::{Rc, Weak};

use ropey::{str_utils::byte_to_char_idx, Rope, RopeSlice};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

/// Visible handle to text buffer
#[derive(Clone)]
pub(crate) struct Buffer {
    inner: Rc<RefCell<BufferInner>>,
}

impl Buffer {
    /// Create empty text buffer
    pub(crate) fn empty() -> Buffer {
        Buffer {
            inner: Rc::new(RefCell::new(BufferInner::empty())),
        }
    }

    /// Load buffer contents from file
    pub(crate) fn from_file(path: &str) -> IOResult<Buffer> {
        BufferInner::from_file(path).map(|tbi| Buffer {
            inner: Rc::new(RefCell::new(tbi)),
        })
    }

    /// Get position indicator at start of line number
    pub(crate) fn get_pos_at_line(&self, linum: usize) -> BufferPos {
        let inner = &*self.inner.borrow();
        if linum >= inner.data.len_lines() {
            let cidx = inner.data.len_chars();
            let linum = inner.data.char_to_line(cidx);
            let linoff = cidx - inner.data.line_to_char(linum);
            BufferPos {
                char_idx: cidx,
                line_num: linum,
                line_off: linoff,
                buffer: self,
            }
        } else {
            BufferPos {
                char_idx: inner.data.line_to_char(linum),
                line_num: linum,
                line_off: 0,
                buffer: self,
            }
        }
    }

    /// Add cursor at position
    pub(crate) fn add_cursor_at_pos(&self, pos: &BufferPos) -> BufferCursor {
        let inner = &mut *self.inner.borrow_mut();
        inner.clean_cursors();
        let idx = inner.cursors.binary_search_by_key(&pos.char_idx, |weak| {
            (&*weak.inner.upgrade().unwrap().borrow()).char_idx
        });
        match idx {
            Ok(idx) => BufferCursor {
                inner: inner.cursors[idx].inner.upgrade().unwrap(),
            },
            Err(idx) => {
                let ret = BufferCursor {
                    inner: Rc::new(RefCell::new(BufferCursorInner {
                        char_idx: pos.char_idx,
                        line_num: pos.line_num,
                        line_off: pos.line_off,
                    })),
                };
                inner.cursors.insert(
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
        let b_inner = &mut *self.inner.borrow_mut();

        // Insert character
        let char_idx = {
            let c_inner = &*cursor.inner.borrow();
            b_inner.data.insert_char(c_inner.char_idx, c);
            c_inner.char_idx
        };

        // Update cursors after current cursor position
        b_inner.clean_cursors();
        let idx = b_inner.cursors.binary_search_by_key(&char_idx, |weak| {
            (&*weak.inner.upgrade().unwrap().borrow()).char_idx
        });
        let idx = match idx {
            Ok(idx) => {
                let strong = b_inner.cursors[idx].inner.upgrade().unwrap();
                let inner = &mut *strong.borrow_mut();
                inner.char_idx += 1;
                let slice = b_inner.data.slice(..);
                if !is_grapheme_boundary(&slice, inner.char_idx) {
                    inner.char_idx = next_grapheme_boundary(&slice, inner.char_idx);
                }
                inner.line_num = b_inner.data.char_to_line(inner.char_idx);
                inner.line_off = inner.char_idx - b_inner.data.line_to_char(inner.line_num);
                idx
            }
            Err(_) => panic!("cursor not found in buffer"),
        };
        for i in (idx + 1)..b_inner.cursors.len() {
            let strong = b_inner.cursors[i].inner.upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            inner.char_idx += 1;
            inner.line_num = b_inner.data.char_to_line(inner.char_idx);
            inner.line_off = inner.char_idx - b_inner.data.line_to_char(inner.line_num);
        }
    }

    /// Insert string at given cursor position
    pub(crate) fn insert_str(&mut self, cursor: &mut BufferCursor, s: &str) {
        let b_inner = &mut *self.inner.borrow_mut();

        // Get char count
        let ccount = s.chars().count();

        // Insert character
        let char_idx = {
            let c_inner = &*cursor.inner.borrow();
            b_inner.data.insert(c_inner.char_idx, s);
            c_inner.char_idx
        };

        // Update cursors after current cursor position
        b_inner.clean_cursors();
        let idx = b_inner.cursors.binary_search_by_key(&char_idx, |weak| {
            (&*weak.inner.upgrade().unwrap().borrow()).char_idx
        });
        let idx = match idx {
            Ok(idx) => {
                let strong = b_inner.cursors[idx].inner.upgrade().unwrap();
                let inner = &mut *strong.borrow_mut();
                inner.char_idx += ccount;
                let slice = b_inner.data.slice(..);
                if !is_grapheme_boundary(&slice, inner.char_idx) {
                    inner.char_idx = next_grapheme_boundary(&slice, inner.char_idx);
                }
                inner.line_num = b_inner.data.char_to_line(inner.char_idx);
                inner.line_off = inner.char_idx - b_inner.data.line_to_char(inner.line_num);
                idx
            }
            Err(_) => panic!("cursor not found in buffer"),
        };
        for i in (idx + 1)..b_inner.cursors.len() {
            let strong = b_inner.cursors[i].inner.upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            inner.char_idx += ccount;
            inner.line_num = b_inner.data.char_to_line(inner.char_idx);
            inner.line_off = inner.char_idx - b_inner.data.line_to_char(inner.line_num);
        }
    }
}

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
pub(crate) struct BufferPos<'a> {
    char_idx: usize,
    line_num: usize,
    line_off: usize,
    buffer: &'a Buffer,
}

impl<'a> BufferPos<'a> {}

// Actual text storage
struct BufferInner {
    data: Rope,
    cursors: Vec<BufferCursorWeak>,
}

impl BufferInner {
    // Create empty buffer
    fn empty() -> BufferInner {
        BufferInner {
            data: Rope::new(),
            cursors: Vec::new(),
        }
    }

    // Create buffer from file
    fn from_file(path: &str) -> IOResult<BufferInner> {
        File::open(path)
            .and_then(|f| Rope::from_reader(f))
            .map(|r| BufferInner {
                data: r,
                cursors: Vec::new(),
            })
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

        assert_eq!(
            &format!("{}", (&*buffer.inner.borrow()).data),
            "hello world"
        );
    }
}
