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

static TEXT_FG_COLOR: Color = Color::new(64, 64, 64, 255);
static TEXT_SIZE: f32 = 8.0;

/// A cursor into the buffer. The buffer maintains references to all cursors, so they are
/// updated on editing the buffer
pub(crate) struct BufferCursor {
    inner: Rc<RefCell<BufferCursorInner>>,
}

impl BufferCursor {
    pub(crate) fn line_num(&self) -> usize {
        (&*self.inner.borrow()).line_num
    }

    pub(crate) fn line_gidx(&self) -> usize {
        (&*self.inner.borrow()).line_gidx
    }
}

struct BufferCursorInner {
    char_idx: usize,
    line_num: usize,
    line_cidx: usize,
    line_gidx: usize,
}

/// A location within a buffer. This is invalidated on editing the buffer
pub(crate) struct BufferPos {
    char_idx: usize,
    line_num: usize,
    line_cidx: usize,
    line_gidx: usize,
}

impl BufferPos {
    pub(crate) fn line_num(&self) -> usize {
        self.line_num
    }
}

// Actual text storage
pub(crate) struct Buffer {
    data: Rope,
    tabsize: usize,
    cursors: Vec<Weak<RefCell<BufferCursorInner>>>,
}

impl Buffer {
    /// Create empty text buffer
    pub(crate) fn empty(tabsize: usize) -> Buffer {
        Buffer {
            data: Rope::new(),
            cursors: Vec::new(),
            tabsize: tabsize,
        }
    }

    /// Create buffer from file
    pub(crate) fn from_file(path: &str, tabsize: usize) -> IOResult<Buffer> {
        File::open(path)
            .and_then(|f| Rope::from_reader(f))
            .map(|r| Buffer {
                data: r,
                cursors: Vec::new(),
                tabsize: tabsize,
            })
    }

    /// Set buffer tabsize
    pub(crate) fn set_tabsize(&mut self, tabsize: usize) {
        self.tabsize = tabsize;
        // TODO: Update all cursors
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
                line_cidx: linoff,
                line_gidx: gidx_from_cidx(&self.data.line(linum), linoff, self.tabsize),
            }
        } else {
            BufferPos {
                char_idx: self.data.line_to_char(linum),
                line_num: linum,
                line_cidx: 0,
                line_gidx: 0,
            }
        }
    }

    /// Get formatted lines from point
    pub(crate) fn fmt_lines_from_pos<'a>(&'a self, pos: &BufferPos) -> BufferFmtLineIter<'a> {
        BufferFmtLineIter {
            lines: self.data.lines_at(pos.line_num),
            tabsize: self.tabsize,
        }
    }

    /// Add cursor at position
    pub(crate) fn add_cursor_at_pos(&mut self, pos: &BufferPos) -> BufferCursor {
        self.clean_cursors();
        let idx = self.cursors.binary_search_by_key(&pos.char_idx, |weak| {
            (&*weak.upgrade().unwrap().borrow()).char_idx
        });
        let idx = match idx {
            Ok(idx) => idx,
            Err(idx) => idx,
        };
        let strong = Rc::new(RefCell::new(BufferCursorInner {
            char_idx: pos.char_idx,
            line_num: pos.line_num,
            line_cidx: pos.line_cidx,
            line_gidx: pos.line_gidx,
        }));
        self.cursors.insert(idx, Rc::downgrade(&strong));
        BufferCursor { inner: strong }
    }

    /// Insert character at given cursor position
    pub(crate) fn insert_char(&mut self, cursor: &mut BufferCursor, c: char) {
        // Insert character
        let char_idx = {
            let cursor = &*cursor.inner.borrow();
            self.data.insert_char(cursor.char_idx, c);
            cursor.char_idx
        };

        // Update cursors after current cursor position
        self.clean_cursors();
        let idx = self.cursors.binary_search_by_key(&char_idx, |weak| {
            (&*weak.upgrade().unwrap().borrow()).char_idx
        });
        let idx = match idx {
            Ok(idx) => {
                let cursor = &mut *cursor.inner.borrow_mut();
                cursor.char_idx += 1;
                let slice = self.data.slice(..);
                if !is_grapheme_boundary(&slice, cursor.char_idx) {
                    cursor.char_idx = next_grapheme_boundary(&slice, cursor.char_idx);
                }
                cursor.line_num = self.data.char_to_line(cursor.char_idx);
                cursor.line_cidx = cursor.char_idx - self.data.line_to_char(cursor.line_num);
                let line = self.data.line(cursor.line_num);
                cursor.line_gidx = gidx_from_cidx(&line, cursor.line_cidx, self.tabsize);
                idx
            }
            Err(_) => panic!("cursor not found in buffer"),
        };
        for i in (idx + 1)..self.cursors.len() {
            let strong = self.cursors[i].upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            inner.char_idx += 1;
            inner.line_num = self.data.char_to_line(inner.char_idx);
            inner.line_cidx = inner.char_idx - self.data.line_to_char(inner.line_num);
            let line = self.data.line(inner.line_num);
            inner.line_gidx = gidx_from_cidx(&line, inner.line_cidx, self.tabsize);
        }
    }

    /// Insert string at given cursor position
    pub(crate) fn insert_str(&mut self, cursor: &mut BufferCursor, s: &str) {
        // Get char count
        let ccount = s.chars().count();

        // Insert character
        let char_idx = {
            let cursor = &*cursor.inner.borrow();
            self.data.insert(cursor.char_idx, s);
            cursor.char_idx
        };

        // Update cursors after current cursor position
        self.clean_cursors();
        let idx = self.cursors.binary_search_by_key(&char_idx, |weak| {
            (&*weak.upgrade().unwrap().borrow()).char_idx
        });
        let idx = match idx {
            Ok(idx) => {
                let cursor = &mut *cursor.inner.borrow_mut();
                cursor.char_idx += ccount;
                let slice = self.data.slice(..);
                if !is_grapheme_boundary(&slice, cursor.char_idx) {
                    cursor.char_idx = next_grapheme_boundary(&slice, cursor.char_idx);
                }
                cursor.line_num = self.data.char_to_line(cursor.char_idx);
                cursor.line_cidx = cursor.char_idx - self.data.line_to_char(cursor.line_num);
                let line = self.data.line(cursor.line_num);
                cursor.line_gidx = gidx_from_cidx(&line, cursor.line_cidx, self.tabsize);
                idx
            }
            Err(_) => panic!("cursor not found in buffer"),
        };
        for i in (idx + 1)..self.cursors.len() {
            let strong = self.cursors[i].upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            inner.char_idx += ccount;
            inner.line_num = self.data.char_to_line(inner.char_idx);
            inner.line_cidx = inner.char_idx - self.data.line_to_char(inner.line_num);
            let line = self.data.line(inner.line_num);
            inner.line_gidx = gidx_from_cidx(&line, inner.line_cidx, self.tabsize);
        }
    }

    pub(super) fn move_cursor_left(&mut self, cursor: &mut BufferCursor, n: usize) {
        let cursor = &mut *cursor.inner.borrow_mut();
        if cursor.line_cidx <= n {
            cursor.line_cidx = 0;
            cursor.line_gidx = 0;
        } else {
            cursor.line_cidx -= n;
            let line = self.data.line(cursor.line_num);
            let (cidx, gidx) = cidx_gidx_from_cidx(&line, cursor.line_cidx, self.tabsize);
            cursor.line_cidx = cidx;
            cursor.line_gidx = gidx;
        }
    }

    // TODO: Evaluate if we should do this on demand only
    fn clean_cursors(&mut self) {
        self.cursors.retain(|weak| weak.strong_count() > 0);
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
    tabsize: usize,
}

impl<'a> BufferFmtLineIter<'a> {
    pub(crate) fn prev<'b>(&mut self, buf: &'b mut String) -> Option<TextLine<'b>> {
        self.lines.prev().map(move |l| {
            expand_line(l, self.tabsize, buf);
            TextLine(vec![TextSpan::new(
                buf,
                TextSize::from_f32(TEXT_SIZE),
                TextStyle::new(TextWeight::Medium, TextSlant::Roman),
                TEXT_FG_COLOR,
                TextPitch::Fixed,
                None,
            )])
        })
    }

    pub(crate) fn next<'b>(&mut self, buf: &'b mut String) -> Option<TextLine<'b>> {
        self.lines.next().map(move |l| {
            expand_line(l, self.tabsize, buf);
            TextLine(vec![TextSpan::new(
                buf,
                TextSize::from_f32(TEXT_SIZE),
                TextStyle::new(TextWeight::Medium, TextSlant::Roman),
                TEXT_FG_COLOR,
                TextPitch::Fixed,
                None,
            )])
        })
    }
}

fn expand_line(slice: RopeSlice, tabsize: usize, buf: &mut String) {
    buf.clear();
    let slice = trim_newlines(slice);
    if slice.len_chars() == 0 {
        buf.push(' ');
    } else {
        let mut x = 0;
        for c in slice.chars() {
            match c {
                '\t' => {
                    let next = (x / tabsize) * tabsize + tabsize;
                    while x < next {
                        x += 1;
                        buf.push(' ');
                    }
                }
                c => {
                    buf.push(c);
                    x += 1;
                }
            }
        }
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

fn gidx_from_cidx(line: &RopeSlice, cidx: usize, tabsize: usize) -> usize {
    let (mut gidx, mut ccount) = (0, 0);
    for g in RopeGraphemes::new(line) {
        ccount += g.chars().count();
        if ccount > cidx {
            return gidx;
        }
        if g == "\t" {
            gidx = (gidx / tabsize) * tabsize + tabsize;
        } else {
            gidx += 1;
        }
    }
    gidx
}

fn cidx_gidx_from_cidx(slice: &RopeSlice, cidx: usize, tabsize: usize) -> (usize, usize) {
    let (mut gidx, mut ccount) = (0, 0);
    for g in RopeGraphemes::new(slice) {
        let count_here = g.chars().count();
        if ccount + count_here > cidx {
            return (ccount, gidx);
        }
        ccount += count_here;
        if g == "\t" {
            gidx = (gidx / tabsize) * tabsize + tabsize;
        } else {
            gidx += 1;
        }
    }
    (ccount, gidx)
}
