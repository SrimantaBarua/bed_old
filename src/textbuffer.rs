// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::Result as IOResult;
use std::rc::{Rc, Weak};

use euclid::Size2D;
use ropey::{iter::Chunks, str_utils::byte_to_char_idx, Rope, RopeSlice};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use crate::types::{Color, TextPitch, TextSize, TextSlant, TextStyle, TextWeight, DPI};
use crate::ui::font::{FaceKey, FontCore};
use crate::ui::text::{ShapedTextLine, TextLine, TextSpan};

static GUTTER_FG_COLOR: Color = Color::new(196, 196, 196, 255);
static GUTTER_TEXT_SIZE: f32 = 7.0;
static TEXT_FG_COLOR: Color = Color::new(96, 96, 96, 255);
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

    pub(crate) fn set_past_end(&mut self, val: bool) {
        (&mut *self.inner.borrow_mut()).past_end = val;
    }
}

struct BufferCursorInner {
    char_idx: usize,
    line_num: usize,
    line_cidx: usize,
    line_gidx: usize,
    line_global_x: usize,
    past_end: bool,
    view_id: usize,
}

impl BufferCursorInner {
    fn sync_from_and_udpate_char_idx_left(&mut self, data: &Rope, tabsize: usize) {
        self.line_num = data.char_to_line(self.char_idx);
        self.line_cidx = self.char_idx - data.line_to_char(self.line_num);
        self.sync_line_cidx_gidx_left(data, tabsize);
    }

    fn sync_from_and_udpate_char_idx_right(&mut self, data: &Rope, tabsize: usize) {
        self.line_num = data.char_to_line(self.char_idx);
        self.line_cidx = self.char_idx - data.line_to_char(self.line_num);
        self.sync_line_cidx_gidx_right(data, tabsize);
    }

    fn sync_line_cidx_gidx_left(&mut self, data: &Rope, tabsize: usize) {
        let trimmed = trim_newlines(data.line(self.line_num));
        let len_chars = trimmed.len_chars();
        if self.line_cidx >= len_chars {
            self.line_cidx = len_chars;
            if !self.past_end && self.line_cidx > 0 {
                self.line_cidx -= 1;
            }
        }
        let (cidx, gidx) = cidx_gidx_from_cidx(&trimmed, self.line_cidx, tabsize);
        self.line_cidx = cidx;
        self.line_gidx = gidx;
        self.line_global_x = self.line_gidx;
        self.char_idx = data.line_to_char(self.line_num) + self.line_cidx;
    }

    fn sync_line_cidx_gidx_right(&mut self, data: &Rope, tabsize: usize) {
        let trimmed = trim_newlines(data.line(self.line_num));
        let len_chars = trimmed.len_chars();
        if self.line_cidx > len_chars {
            self.line_cidx = len_chars;
        }
        if !is_grapheme_boundary(&trimmed, self.line_cidx) {
            self.line_cidx = next_grapheme_boundary(&trimmed, self.line_cidx);
        }
        if !self.past_end && self.line_cidx == len_chars && self.line_cidx > 0 {
            self.line_cidx -= 1;
        }
        let (cidx, gidx) = cidx_gidx_from_cidx(&trimmed, self.line_cidx, tabsize);
        self.line_cidx = cidx;
        self.line_gidx = gidx;
        self.line_global_x = self.line_gidx;
        self.char_idx = data.line_to_char(self.line_num) + self.line_cidx;
    }

    fn sync_from_global_x(&mut self, data: &Rope, tabsize: usize) {
        let trimmed = trim_newlines(data.line(self.line_num));
        let (cidx, gidx) =
            cidx_gidx_from_global_x(&trimmed, self.line_global_x, tabsize, self.past_end);
        self.line_cidx = cidx;
        self.line_gidx = gidx;
        self.char_idx = data.line_to_char(self.line_num) + self.line_cidx;
    }

    fn sync_from_gidx(&mut self, data: &Rope, tabsize: usize) {
        let trimmed = trim_newlines(data.line(self.line_num));
        let (cidx, gidx) = cidx_gidx_from_gidx(&trimmed, self.line_gidx, tabsize, self.past_end);
        self.line_cidx = cidx;
        self.line_gidx = gidx;
        self.line_global_x = self.line_gidx;
        self.char_idx = data.line_to_char(self.line_num) + self.line_cidx;
    }
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
    fmtbuf: String,
    data: Rope,
    tabsize: usize,
    path: Option<String>,
    cursors: HashMap<usize, Weak<RefCell<BufferCursorInner>>>,
    fixed_face: FaceKey,
    variable_face: FaceKey,
    font_core: Rc<RefCell<FontCore>>,
    dpi_shaped_lines: Vec<(Size2D<u32, DPI>, Vec<ShapedTextLine>, Vec<ShapedTextLine>)>,
}

impl Buffer {
    /// Create empty text buffer
    pub(crate) fn empty(
        tabsize: usize,
        initial_dpi: Size2D<u32, DPI>,
        fixed_face: FaceKey,
        variable_face: FaceKey,
        font_core: Rc<RefCell<FontCore>>,
    ) -> Buffer {
        let mut ret = Buffer {
            fmtbuf: String::new(),
            data: Rope::new(),
            cursors: HashMap::new(),
            path: None,
            tabsize: tabsize,
            dpi_shaped_lines: vec![(initial_dpi, Vec::new(), Vec::new())],
            fixed_face: fixed_face,
            variable_face: variable_face,
            font_core: font_core,
        };
        ret.format_lines_from(0, None);
        ret
    }

    /// Create buffer from file
    pub(crate) fn from_file(
        path: &str,
        tabsize: usize,
        initial_dpi: Size2D<u32, DPI>,
        fixed_face: FaceKey,
        variable_face: FaceKey,
        font_core: Rc<RefCell<FontCore>>,
    ) -> IOResult<Buffer> {
        File::open(path)
            .and_then(|f| Rope::from_reader(f))
            .map(|r| {
                let mut ret = Buffer {
                    fmtbuf: String::new(),
                    data: r,
                    cursors: HashMap::new(),
                    path: Some(path.to_owned()),
                    tabsize: tabsize,
                    dpi_shaped_lines: vec![(initial_dpi, Vec::new(), Vec::new())],
                    fixed_face: fixed_face,
                    variable_face: variable_face,
                    font_core: font_core,
                };
                ret.format_lines_from(0, None);
                ret
            })
    }

    /// Reload buffer contents and reset all cursors
    pub(crate) fn reload_from_file(&mut self, dpi: Size2D<u32, DPI>) -> IOResult<()> {
        if let Some(path) = &self.path {
            File::open(path)
                .and_then(|f| Rope::from_reader(f))
                .map(|r| {
                    self.data = r;
                    self.clean_cursors();
                    let len_chars = self.data.len_chars();
                    for (_, weak) in self.cursors.iter_mut() {
                        let strong = weak.upgrade().unwrap();
                        let inner = &mut *strong.borrow_mut();
                        if inner.char_idx >= len_chars {
                            inner.char_idx = len_chars;
                            inner.sync_from_and_udpate_char_idx_left(&self.data, self.tabsize);
                        }
                    }
                    let mut found = false;
                    for (d, _, t) in &mut self.dpi_shaped_lines {
                        t.clear();
                        if *d == dpi {
                            found = true;
                        }
                    }
                    if !found {
                        self.dpi_shaped_lines.push((dpi, Vec::new(), Vec::new()));
                    }
                    self.format_lines_from(0, None);
                })
        } else {
            unreachable!()
        }
    }

    /// Set buffer tabsize
    pub(crate) fn set_tabsize(&mut self, tabsize: usize) {
        self.tabsize = tabsize;
        // TODO: Update all cursors
        // TODO: Re-format lines
    }

    /// Number of lines in buffer
    pub(crate) fn len_lines(&self) -> usize {
        self.data.len_lines()
    }

    /// Reference to shaped line numbers and line text given DPI
    pub(crate) fn shaped_data(
        &self,
        dpi: Size2D<u32, DPI>,
    ) -> Option<(&[ShapedTextLine], &[ShapedTextLine])> {
        self.dpi_shaped_lines
            .iter()
            .filter_map(|(x, l, t)| {
                if *x == dpi {
                    Some((l.as_ref(), t.as_ref()))
                } else {
                    None
                }
            })
            .next()
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

    /// Add cursor at position
    pub(crate) fn add_cursor_at_pos(
        &mut self,
        view_id: usize,
        pos: &BufferPos,
        past_end: bool,
    ) -> BufferCursor {
        self.clean_cursors_except(view_id);
        let mut inner = BufferCursorInner {
            char_idx: pos.char_idx,
            line_num: pos.line_num,
            line_cidx: pos.line_cidx,
            line_gidx: pos.line_gidx,
            line_global_x: pos.line_gidx,
            past_end: past_end,
            view_id: view_id,
        };
        if !inner.past_end {
            let trimmed = trim_newlines(self.data.line(inner.line_num));
            if inner.line_cidx == trimmed.len_chars() && inner.line_cidx > 0 {
                let (cidx, gidx) = cidx_gidx_from_cidx(&trimmed, inner.line_cidx - 1, self.tabsize);
                inner.line_cidx = cidx;
                inner.line_gidx = gidx;
                inner.line_global_x = inner.line_gidx;
                inner.char_idx = self.data.line_to_char(inner.line_num) + inner.line_cidx;
            }
        }
        let strong = Rc::new(RefCell::new(inner));
        self.cursors.insert(view_id, Rc::downgrade(&strong));
        BufferCursor { inner: strong }
    }

    /// Delete to the left of cursor
    pub(crate) fn delete_left(&mut self, cursor: &mut BufferCursor, n: usize) {
        // Delete contents and re-format
        let (start_cidx, end_cidx, view_id) = {
            let cursor = &mut *cursor.inner.borrow_mut();
            if cursor.char_idx == 0 {
                return;
            }
            let cidx = if cursor.char_idx <= n {
                0
            } else {
                cursor.char_idx - n
            };
            // Calculate formatting replace range
            let start_line = self.data.char_to_line(cidx);
            let mut end_line = cursor.line_num;
            let len_chars = trim_newlines(self.data.line(cursor.line_num)).len_chars();
            if cursor.line_cidx >= len_chars {
                end_line += 1;
            }
            // Delete
            self.data.remove(cidx..cursor.char_idx);
            // Reformat
            for (_, _, t) in &mut self.dpi_shaped_lines {
                if end_line > start_line + 1 {
                    t.drain((start_line + 1)..end_line);
                }
            }
            self.format_lines_from(start_line, None);
            // Metrics to place cursors
            (cidx, cursor.char_idx, cursor.view_id)
        };

        // Update cursors after current cursor position (inclusive of current cursor)
        self.clean_cursors_except(view_id);

        for (_, weak) in self.cursors.iter_mut() {
            let strong = weak.upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            if inner.char_idx < start_cidx {
                continue;
            }
            if inner.char_idx <= end_cidx {
                inner.char_idx = start_cidx;
            } else {
                inner.char_idx -= end_cidx - start_cidx;
            }
            inner.sync_from_and_udpate_char_idx_left(&self.data, self.tabsize);
        }
    }

    /// Delete to the right of cursor
    pub(crate) fn delete_right(&mut self, cursor: &mut BufferCursor, n: usize) {
        // Delete contents and reformat
        let (start_cidx, end_cidx, view_id) = {
            let cursor = &mut *cursor.inner.borrow_mut();
            let len_chars = self.data.len_chars();
            let final_cidx = if cursor.char_idx + n >= len_chars {
                len_chars
            } else {
                cursor.char_idx + n
            };
            if final_cidx == cursor.char_idx {
                return;
            }
            // Calculate formatting replace range
            let start_line = self.data.char_to_line(cursor.char_idx);
            let mut end_line = self.data.char_to_line(final_cidx);
            let len_chars = trim_newlines(self.data.line(final_cidx)).len_chars();
            if final_cidx - self.data.line_to_char(end_line) >= len_chars {
                end_line += 1;
            }
            // Delete
            self.data.remove(cursor.char_idx..final_cidx);
            // Reformat
            for (_, _, t) in &mut self.dpi_shaped_lines {
                if end_line > start_line + 1 {
                    t.drain((start_line + 1)..end_line);
                }
            }
            self.format_lines_from(start_line, None);
            // Metrics to place cursors
            (cursor.char_idx, final_cidx, cursor.view_id)
        };

        // Update cursors after current cursor position (inclusive of current cursor)
        self.clean_cursors_except(view_id);

        for (_, weak) in self.cursors.iter_mut() {
            let strong = weak.upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            if inner.char_idx < start_cidx {
                continue;
            }
            if inner.char_idx <= end_cidx {
                inner.char_idx = start_cidx;
            } else {
                inner.char_idx -= end_cidx - start_cidx;
            }
            inner.sync_from_and_udpate_char_idx_left(&self.data, self.tabsize);
        }

        // Re-format lines
        let start_line = self.data.char_to_line(start_cidx);
        let mut end_line = self.data.char_to_line(end_cidx);
        let trimmed = trim_newlines(self.data.line(end_line));
        if end_cidx >= trimmed.len_chars() {
            end_line += 1;
        }
        for (_, _, t) in &mut self.dpi_shaped_lines {
            t[start_line] = ShapedTextLine::default();
            if end_line > start_line {
                t.drain((start_line + 1)..end_line);
            }
        }
        self.format_lines_from(self.data.char_to_line(start_cidx), None);
    }

    /// Delete to start of line
    pub(crate) fn delete_to_line_start(&mut self, cursor: &mut BufferCursor) {
        // Delete contents
        let cursor = &mut *cursor.inner.borrow_mut();
        let cidx = self.data.line_to_char(cursor.line_num);
        let diff = cursor.char_idx - cidx;
        if diff == 0 {
            return;
        }
        self.data.remove(cidx..cursor.char_idx);
        cursor.char_idx = cidx;
        cursor.line_cidx = 0;
        cursor.line_gidx = 0;
        cursor.line_global_x = 0;

        // Update cursors after current cursor position
        self.clean_cursors_except(cursor.view_id);

        for (&k, weak) in self.cursors.iter_mut() {
            if k == cursor.view_id {
                continue;
            }
            let strong = weak.upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            if inner.line_num <= cursor.line_num {
                continue;
            }
            if inner.line_num == cursor.line_num {
                if inner.line_cidx <= diff {
                    inner.char_idx = cidx;
                    inner.line_cidx = 0;
                    inner.line_gidx = 0;
                    inner.line_global_x = 0;
                } else {
                    inner.char_idx -= diff;
                    inner.line_cidx -= diff;
                    inner.sync_line_cidx_gidx_left(&self.data, self.tabsize);
                }
            } else {
                inner.char_idx -= diff;
            }
        }

        // Re-format lines
        self.format_lines_from(cursor.line_num, None);
    }

    /// Delete to the end of line
    pub(crate) fn delete_to_line_end(&mut self, cursor: &mut BufferCursor) {
        // Delete contents
        let (linum, diff, view_id, char_idx) = {
            let cursor = &mut *cursor.inner.borrow_mut();
            let len_chars = trim_newlines(self.data.line(cursor.line_num)).len_chars();
            let diff = len_chars - cursor.line_cidx;
            if diff == 0 {
                return;
            }
            self.data.remove(cursor.char_idx..(cursor.char_idx + diff));
            (cursor.line_num, diff, cursor.view_id, cursor.char_idx)
        };

        // Update cursors after current cursor position
        self.clean_cursors_except(view_id);
        for (_, weak) in self.cursors.iter_mut() {
            let strong = weak.upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            if inner.char_idx < char_idx {
                continue;
            }
            if inner.line_num == linum {
                inner.sync_line_cidx_gidx_left(&self.data, self.tabsize);
            } else {
                inner.char_idx -= diff;
            }
        }

        // Re-format lines
        self.format_lines_from(linum, None);
    }

    pub(crate) fn delete_lines(&mut self, cursor: &mut BufferCursor, nlines: usize) {
        let (start, end, linum, nlines, view_id) = {
            let cursor = &mut *cursor.inner.borrow_mut();
            let start = cursor.char_idx - cursor.line_cidx;
            if start == self.data.len_chars() {
                return;
            }
            let (nlines, end) = if cursor.line_num + nlines > self.data.len_lines() {
                (
                    self.data.len_lines() - cursor.line_num,
                    self.data.len_chars(),
                )
            } else {
                (nlines, self.data.line_to_char(cursor.line_num + nlines))
            };
            self.data.remove(start..end);
            (start, end, cursor.line_num, nlines, cursor.view_id)
        };

        // Update cursors after current cursor position
        self.clean_cursors_except(view_id);
        for (_, weak) in self.cursors.iter_mut() {
            let strong = weak.upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            if inner.char_idx <= start {
                continue;
            }
            if inner.char_idx >= end {
                inner.char_idx -= end - start;
                inner.line_num -= nlines;
                continue;
            }
            inner.char_idx = start;
            inner.line_num = linum;
            inner.line_cidx = 0;
            inner.line_gidx = 0;
            inner.line_global_x = 0;
        }

        // Reformat
        for (_, _, t) in &mut self.dpi_shaped_lines {
            t.drain(linum..(linum + nlines));
        }
        self.format_lines_from(linum, None);
    }

    pub(crate) fn delete_lines_up(&mut self, cursor: &mut BufferCursor, mut nlines: usize) {
        {
            let cursor = &mut *cursor.inner.borrow_mut();
            if cursor.line_num < nlines {
                nlines = cursor.line_num;
            }
            cursor.line_num -= nlines;
            cursor.line_cidx = 0;
            cursor.char_idx = self.data.line_to_char(cursor.line_num);
        }
        self.delete_lines(cursor, nlines + 1);
    }

    pub(crate) fn delete_lines_down(&mut self, cursor: &mut BufferCursor, nlines: usize) {
        self.delete_lines(cursor, nlines + 1);
    }

    pub(crate) fn delete_to_line(&mut self, cursor: &mut BufferCursor, linum: usize) {
        let nlines = {
            let cursor = &mut *cursor.inner.borrow_mut();
            linum as isize - cursor.line_num as isize
        };
        if nlines < 0 {
            self.delete_lines_up(cursor, (-nlines) as usize);
        } else {
            self.delete_lines_down(cursor, nlines as usize);
        }
    }

    pub(crate) fn delete_to_last_line(&mut self, cursor: &mut BufferCursor) {
        self.delete_lines(cursor, self.data.len_lines());
    }

    /// Insert character at given cursor position
    pub(crate) fn insert_char(&mut self, cursor: &mut BufferCursor, c: char) {
        let (old_char_idx, view_id) = {
            let cursor = &mut *cursor.inner.borrow_mut();
            self.data.insert_char(cursor.char_idx, c);
            (cursor.char_idx, cursor.view_id)
        };

        // Update cursors after current cursor position (inclusive of current cursor)
        self.clean_cursors_except(view_id);

        for (&k, weak) in self.cursors.iter_mut() {
            let strong = weak.upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            if inner.char_idx < old_char_idx {
                continue;
            }
            if inner.char_idx == old_char_idx && k != view_id {
                inner.sync_line_cidx_gidx_right(&self.data, self.tabsize);
                continue;
            }
            inner.char_idx += 1;
            inner.sync_from_and_udpate_char_idx_right(&self.data, self.tabsize);
        }

        // Reformat
        let linum = self.data.char_to_line(old_char_idx);
        let mut end = None;
        if c == '\n' {
            for (_, _, t) in &mut self.dpi_shaped_lines {
                t.insert(linum + 1, ShapedTextLine::default());
            }
            end = Some(linum + 1);
        }
        self.format_lines_from(linum, end);
    }

    /// Insert string at given cursor position
    pub(crate) fn insert_str(&mut self, cursor: &mut BufferCursor, s: &str) {
        let ccount = s.chars().count();
        let (old_char_idx, view_id) = {
            let cursor = &*cursor.inner.borrow();
            (cursor.char_idx, cursor.view_id)
        };

        // Insert string
        self.data.insert(old_char_idx, s);

        // Update cursors after current cursor position
        self.clean_cursors_except(view_id);

        for (&k, weak) in self.cursors.iter_mut() {
            let strong = weak.upgrade().unwrap();
            let inner = &mut *strong.borrow_mut();
            if inner.char_idx < old_char_idx {
                continue;
            }
            if inner.char_idx == old_char_idx && k != view_id {
                inner.sync_line_cidx_gidx_right(&self.data, self.tabsize);
                continue;
            }
            inner.char_idx += ccount;
            inner.sync_from_and_udpate_char_idx_right(&self.data, self.tabsize);
        }

        // Reformat
        let linum = self.data.char_to_line(old_char_idx);
        let end_line = self.data.char_to_line(old_char_idx + ccount);
        for (_, _, t) in &mut self.dpi_shaped_lines {
            for _ in linum..end_line {
                t.insert(linum + 1, ShapedTextLine::default());
            }
        }
        self.format_lines_from(linum, Some(end_line));
    }

    /// Move cursor to given line number and gidx
    pub(crate) fn move_cursor_to_linum_gidx(
        &mut self,
        cursor: &mut BufferCursor,
        mut linum: usize,
        gidx: usize,
    ) {
        let len_lines = self.data.len_lines();
        if linum >= len_lines {
            linum = len_lines;
            if len_lines > 0 {
                linum -= 1;
            }
        }
        let cursor = &mut *cursor.inner.borrow_mut();
        cursor.line_num = linum;
        cursor.line_gidx = gidx;
        cursor.sync_from_gidx(&self.data, self.tabsize);
    }

    /// Move cursor n lines up
    pub(crate) fn move_cursor_up(&mut self, cursor: &mut BufferCursor, n: usize) {
        let cursor = &mut *cursor.inner.borrow_mut();
        if cursor.line_num == 0 {
            cursor.char_idx = 0;
            cursor.line_cidx = 0;
            cursor.line_gidx = 0;
            cursor.line_global_x = 0;
            return;
        }
        if cursor.line_num < n {
            cursor.line_num = 0;
        } else {
            cursor.line_num -= n;
        }
        cursor.sync_from_global_x(&self.data, self.tabsize);
    }

    /// Move cursor n lines down
    pub(crate) fn move_cursor_down(&mut self, cursor: &mut BufferCursor, n: usize) {
        let cursor = &mut *cursor.inner.borrow_mut();
        cursor.line_num += n;
        if cursor.line_num >= self.data.len_lines() {
            cursor.char_idx = self.data.len_chars();
            cursor.sync_from_and_udpate_char_idx_left(&self.data, self.tabsize);
        } else {
            cursor.sync_from_global_x(&self.data, self.tabsize);
        }
    }

    /// Move cursor n chars to the left
    pub(crate) fn move_cursor_left(&mut self, cursor: &mut BufferCursor, n: usize) {
        let cursor = &mut *cursor.inner.borrow_mut();
        if cursor.line_cidx <= n {
            cursor.char_idx -= cursor.line_cidx;
            cursor.line_cidx = 0;
            cursor.line_gidx = 0;
            cursor.line_global_x = 0;
        } else {
            cursor.line_cidx -= n;
            cursor.sync_line_cidx_gidx_left(&self.data, self.tabsize);
        }
    }

    /// Move cursor n chars to the right
    pub(crate) fn move_cursor_right(&mut self, cursor: &mut BufferCursor, n: usize) {
        let cursor = &mut *cursor.inner.borrow_mut();
        cursor.line_cidx += n;
        cursor.sync_line_cidx_gidx_right(&self.data, self.tabsize);
    }

    /// Move cursor to the start of line
    pub(crate) fn move_cursor_start_of_line(&mut self, cursor: &mut BufferCursor) {
        let cursor = &mut *cursor.inner.borrow_mut();
        cursor.char_idx -= cursor.line_cidx;
        cursor.line_cidx = 0;
        cursor.line_gidx = 0;
        cursor.line_global_x = 0;
    }

    /// Move cursor to the end of line
    pub(crate) fn move_cursor_end_of_line(&mut self, cursor: &mut BufferCursor) {
        let cursor = &mut *cursor.inner.borrow_mut();
        let trimmed = trim_newlines(self.data.line(cursor.line_num));
        let mut len_chars = trimmed.len_chars();
        if !cursor.past_end && len_chars > 0 {
            len_chars -= 1;
        }
        let diff = len_chars - cursor.line_cidx;
        cursor.char_idx += diff;
        cursor.line_cidx += diff;
        cursor.line_gidx = gidx_from_cidx(&trimmed, cursor.char_idx, self.tabsize);
        cursor.line_global_x = cursor.line_gidx;
    }

    /// Move cursor to given line number
    pub(crate) fn move_cursor_to_line(&mut self, cursor: &mut BufferCursor, mut linum: usize) {
        let len_lines = self.data.len_lines();
        if linum >= len_lines {
            linum = len_lines;
            if len_lines > 0 {
                linum -= 1;
            }
        }
        let cursor = &mut *cursor.inner.borrow_mut();
        cursor.line_num = linum;
        cursor.sync_from_global_x(&self.data, self.tabsize);
    }

    /// Move cursor to last line
    pub(crate) fn move_cursor_to_last_line(&mut self, cursor: &mut BufferCursor) {
        self.move_cursor_to_line(cursor, self.data.len_lines());
    }

    // TODO: Evaluate if we should do this on demand only
    fn clean_cursors_except(&mut self, view_id: usize) {
        self.cursors
            .retain(|&key, weak| key == view_id || weak.strong_count() > 0);
    }

    fn clean_cursors(&mut self) {
        self.cursors.retain(|_, weak| weak.strong_count() > 0);
    }

    fn format_lines_from(&mut self, start: usize, opt_min_end: Option<usize>) {
        let font_core = &mut *self.font_core.borrow_mut();
        for (dpi, lvec, tvec) in &mut self.dpi_shaped_lines {
            for i in start..self.data.len_lines() {
                let line = self.data.line(i);
                expand_line(line, self.tabsize, &mut self.fmtbuf);
                let fmtline = TextLine(vec![TextSpan::new(
                    &self.fmtbuf,
                    TextSize::from_f32(TEXT_SIZE),
                    TextStyle::new(TextWeight::Medium, TextSlant::Roman),
                    TEXT_FG_COLOR,
                    TextPitch::Fixed,
                    None,
                )]);
                let shaped_line = ShapedTextLine::from_textline(
                    fmtline,
                    self.fixed_face,
                    self.variable_face,
                    font_core,
                    *dpi,
                );
                if i >= tvec.len() {
                    tvec.push(shaped_line);
                } else if tvec[i] != shaped_line {
                    tvec[i] = shaped_line;
                } else {
                    if let Some(min) = opt_min_end {
                        if i < min {
                            continue;
                        }
                    }
                    break;
                }
            }
            for linum in lvec.len()..(tvec.len() + 1) {
                self.fmtbuf.clear();
                write!(&mut self.fmtbuf, "{}", linum).unwrap();
                let fmtspan = TextSpan::new(
                    &self.fmtbuf,
                    TextSize::from_f32(GUTTER_TEXT_SIZE),
                    TextStyle::new(TextWeight::Medium, TextSlant::Roman),
                    GUTTER_FG_COLOR,
                    TextPitch::Fixed,
                    None,
                );
                let shaped_line = ShapedTextLine::from_textstr(
                    fmtspan,
                    self.fixed_face,
                    self.variable_face,
                    font_core,
                    *dpi,
                );
                lvec.push(shaped_line);
            }
            // lvec.truncate(tvec.len() + 1);
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
struct RopeGraphemes<'a> {
    text: RopeSlice<'a>,
    chunks: Chunks<'a>,
    cur_chunk: &'a str,
    cur_chunk_start: usize,
    cursor: GraphemeCursor,
}

impl<'a> RopeGraphemes<'a> {
    fn new<'b>(slice: &RopeSlice<'b>) -> RopeGraphemes<'b> {
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

fn cidx_gidx_from_gidx(
    slice: &RopeSlice,
    gidx: usize,
    tabsize: usize,
    past_end: bool,
) -> (usize, usize) {
    let (mut gcount, mut cidx) = (0, 0);
    let mut len_chars = slice.len_chars();
    if !past_end && len_chars > 0 {
        len_chars -= 1;
    }
    for g in RopeGraphemes::new(slice) {
        if gcount >= gidx || cidx >= len_chars {
            return (cidx, gcount);
        }
        let count_here = g.chars().count();
        if cidx + count_here > len_chars {
            return (cidx, gcount);
        }
        cidx += count_here;
        if g == "\t" {
            gcount = (gcount / tabsize) * tabsize + tabsize;
        } else {
            gcount += 1;
        }
    }
    (cidx, gcount)
}

fn cidx_gidx_from_global_x(
    slice: &RopeSlice,
    global_x: usize,
    tabsize: usize,
    past_end: bool,
) -> (usize, usize) {
    let (mut gidx, mut ccount) = (0, 0);
    let mut len_chars = slice.len_chars();
    if !past_end && len_chars > 0 {
        len_chars -= 1;
    }
    for g in RopeGraphemes::new(slice) {
        if ccount >= len_chars {
            return (ccount, gidx);
        }
        if gidx >= global_x {
            return (ccount, gidx);
        }
        ccount += g.chars().count();
        if g == "\t" {
            gidx = (gidx / tabsize) * tabsize + tabsize;
        } else {
            gidx += 1;
        }
    }
    (ccount, gidx)
}
