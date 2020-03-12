// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::cmp::{max, min};
use std::rc::Rc;

use euclid::{point2, size2, Rect, Size2D};

use crate::textbuffer::{Buffer, BufferCursor};
use crate::types::{Color, PixelSize, TextSize, DPI};

use super::context::ActiveRenderCtx;
use super::font::{FaceKey, FontCore};
use super::text::{ShapedTextLine, TextCursorStyle};

struct View {
    xbase: u32,
    ybase: u32,
    start_line: usize,
    line_numbers: bool,
    relative_number: bool,
    buffer: Rc<RefCell<Buffer>>,
    cursor: BufferCursor,
}

pub(super) struct TextView {
    views: Vec<View>,
    cur_view_idx: usize,
    rect: Rect<u32, PixelSize>,
    background_color: Color,
    fixed_face: FaceKey,
    variable_face: FaceKey,
    gutter_padding: u32,
    gutter_textsize: TextSize,
    gutter_background_color: Color,
    line_numbers: bool,
    relative_number: bool,
    dpi: Size2D<u32, DPI>,
    font_core: Rc<RefCell<FontCore>>,
    cursor_color: Color,
    cursor_style: TextCursorStyle,
}

impl TextView {
    pub(super) fn new(
        buffer: Rc<RefCell<Buffer>>,
        rect: Rect<u32, PixelSize>,
        background_color: Color,
        fixed_face: FaceKey,
        variable_face: FaceKey,
        font_core: Rc<RefCell<FontCore>>,
        dpi: Size2D<u32, DPI>,
        line_numbers: bool,
        relative_number: bool,
        gutter_padding: u32,
        gutter_textsize: TextSize,
        gutter_background_color: Color,
        cursor_color: Color,
        view_id: usize,
    ) -> TextView {
        let cursor = {
            let borrow = &mut *buffer.borrow_mut();
            let pos = borrow.get_pos_at_line(0);
            borrow.add_cursor_at_pos(view_id, &pos, false)
        };
        let views = vec![View {
            xbase: 0,
            ybase: 0,
            start_line: 0,
            line_numbers: line_numbers,
            relative_number: relative_number,
            buffer: buffer,
            cursor: cursor,
        }];
        TextView {
            views: views,
            cur_view_idx: 0,
            rect: rect,
            background_color: background_color,
            fixed_face: fixed_face,
            variable_face: variable_face,
            font_core: font_core,
            dpi: dpi,
            line_numbers: line_numbers,
            relative_number: relative_number,
            gutter_padding: gutter_padding,
            gutter_textsize: gutter_textsize,
            gutter_background_color: gutter_background_color,
            cursor_color: cursor_color,
            cursor_style: TextCursorStyle::Block,
        }
    }

    pub(super) fn add_buffer(&mut self, buffer: Rc<RefCell<Buffer>>, view_id: usize) {
        let cursor = {
            let borrow = &mut *buffer.borrow_mut();
            let pos = borrow.get_pos_at_line(0);
            borrow.add_cursor_at_pos(view_id, &pos, false)
        };
        self.views.push(View {
            xbase: 0,
            ybase: 0,
            start_line: 0,
            line_numbers: self.line_numbers,
            relative_number: self.relative_number,
            buffer: buffer,
            cursor: cursor,
        });
        self.cur_view_idx += 1;
    }

    pub(super) fn prev_buffer(&mut self) {
        if self.cur_view_idx == 0 {
            self.cur_view_idx = self.views.len() - 1;
        } else {
            self.cur_view_idx -= 1;
        }
        self.snap_to_cursor();
    }

    pub(super) fn next_buffer(&mut self) {
        self.cur_view_idx = (self.cur_view_idx + 1) % self.views.len();
        self.snap_to_cursor();
    }

    pub(super) fn set_cursor_style(&mut self, style: TextCursorStyle) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            view.cursor.set_past_end(style == TextCursorStyle::Beam);
            if self.cursor_style == TextCursorStyle::Beam && style == TextCursorStyle::Block {
                buffer.move_cursor_left(&mut view.cursor, 1);
            }
        }
        self.cursor_style = style;
        self.snap_to_cursor();
    }

    pub(super) fn move_cursor_to_point(&mut self, mut point: (i32, i32)) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let cursor_linum = view.cursor.line_num();
            let buffer = &mut *view.buffer.borrow_mut();
            let (shaped_linums, shaped_text) = buffer.shaped_data(self.dpi).unwrap();

            assert!(view.start_line < shaped_text.len());

            if point.0 < 0 {
                point.0 = 0;
            } else if point.0 > self.rect.size.width as i32 {
                point.0 = self.rect.size.width as i32;
            }
            if point.1 < 0 {
                point.1 = 0;
            } else if point.1 > self.rect.size.height as i32 {
                point.1 = self.rect.size.height as i32;
            }

            let gutter_width = if view.line_numbers || view.relative_number {
                shaped_linums[shaped_linums.len() - 1].metrics.width + self.gutter_padding * 2
            } else {
                self.gutter_padding * 2
            };

            point.0 += view.xbase as i32 - gutter_width as i32;
            point.1 += view.ybase as i32;

            let mut total_height = 0;
            let mut linum = view.start_line;

            for (_, _, height, _, _) in LinumTextIter::new(
                shaped_linums,
                shaped_text,
                view.start_line,
                cursor_linum,
                view.line_numbers,
                view.relative_number,
            ) {
                total_height += height as i32;
                if total_height >= point.1 {
                    break;
                }
                linum += 1;
            }
            if linum >= shaped_text.len() {
                linum = shaped_text.len();
                if linum > 0 {
                    linum -= 1;
                }
            }

            let mut x = 0;
            let mut gidx = 0;
            'outer: for span in &shaped_text[linum].spans {
                for cluster in span.clusters() {
                    let num_glyphs = cluster.glyph_infos.len();
                    if num_glyphs % cluster.num_graphemes != 0 {
                        let startx = x;
                        for gi in cluster.glyph_infos {
                            x += gi.advance.width;
                        }
                        if x < point.0 {
                            continue;
                        }
                        let width = x - startx;
                        let grapheme_width = width / cluster.num_graphemes as i32;
                        gidx += width / grapheme_width;
                        break 'outer;
                    } else {
                        let glyphs_per_grapheme = num_glyphs / cluster.num_graphemes;
                        for i in (0..num_glyphs).step_by(glyphs_per_grapheme) {
                            for gi in &cluster.glyph_infos[i..(i + glyphs_per_grapheme)] {
                                x += gi.advance.width;
                                if x >= point.0 {
                                    break 'outer;
                                }
                            }
                            gidx += 1;
                        }
                    }
                }
            }

            buffer.move_cursor_to_linum_gidx(&mut view.cursor, linum, gidx as usize);
        }
        self.snap_to_cursor();
    }

    pub(super) fn move_cursor_down(&mut self, n: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.move_cursor_down(&mut view.cursor, n);
        }
        self.snap_to_cursor();
    }

    pub(super) fn move_cursor_up(&mut self, n: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.move_cursor_up(&mut view.cursor, n);
        }
        self.snap_to_cursor();
    }

    pub(super) fn move_cursor_left(&mut self, n: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.move_cursor_left(&mut view.cursor, n);
        }
        self.snap_to_cursor();
    }

    pub(super) fn move_cursor_right(&mut self, n: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.move_cursor_right(&mut view.cursor, n);
        }
        self.snap_to_cursor();
    }

    pub(super) fn move_cursor_start_of_line(&mut self) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.move_cursor_start_of_line(&mut view.cursor);
        }
        self.snap_to_cursor();
    }

    pub(super) fn move_cursor_end_of_line(&mut self) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.move_cursor_end_of_line(&mut view.cursor);
        }
        self.snap_to_cursor();
    }

    pub(super) fn page_up(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let buffer = &mut *view.buffer.borrow_mut();
        let cursor_linum = view.cursor.line_num();
        let (shaped_linums, shaped_text) = buffer.shaped_data(self.dpi).unwrap();

        view.ybase = 0;
        let linum = if view.start_line == 0 {
            0
        } else {
            let mut total_height = self.rect.size.height;
            let mut iter = LinumTextIter::new(
                shaped_linums,
                shaped_text,
                view.start_line,
                cursor_linum,
                view.line_numbers,
                view.relative_number,
            );
            while let Some((_, _, height, _, _)) = iter.prev() {
                if height > total_height {
                    break;
                }
                total_height -= height;
                view.start_line -= 1;
            }
            let mut linum = view.start_line;
            total_height = 0;
            for (_, _, height, _, _) in LinumTextIter::new(
                shaped_linums,
                shaped_text,
                view.start_line,
                cursor_linum,
                view.line_numbers,
                view.relative_number,
            ) {
                if linum >= cursor_linum || height + total_height >= self.rect.size.height {
                    break;
                }
                total_height += height;
                linum += 1;
            }
            linum - 1
        };
        buffer.move_cursor_to_line(&mut view.cursor, linum);
    }

    pub(super) fn page_down(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let buffer = &mut *view.buffer.borrow_mut();
        let cursor_linum = view.cursor.line_num();
        let (shaped_linums, shaped_text) = buffer.shaped_data(self.dpi).unwrap();

        view.ybase = 0;
        let mut total_height = 0;
        for (_, _, height, _, _) in LinumTextIter::new(
            shaped_linums,
            shaped_text,
            view.start_line,
            cursor_linum,
            view.line_numbers,
            view.relative_number,
        ) {
            if height + total_height >= self.rect.size.height {
                break;
            }
            total_height += height;
            view.start_line += 1;
        }
        if view.start_line > 0 && view.start_line == shaped_text.len() {
            view.start_line -= 1;
        }
        buffer.move_cursor_to_line(&mut view.cursor, view.start_line);
    }

    pub(super) fn go_to_line(&mut self, linum: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.move_cursor_to_line(&mut view.cursor, linum);
        }
        self.snap_to_cursor();
    }

    pub(super) fn go_to_last_line(&mut self) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.move_cursor_to_last_line(&mut view.cursor);
        }
        self.snap_to_cursor();
    }

    pub(super) fn delete_left(&mut self, n: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_left(&mut view.cursor, n);
        }
        self.snap_to_cursor();
    }

    pub(super) fn delete_right(&mut self, n: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_right(&mut view.cursor, n);
        }
        self.snap_to_cursor();
    }

    pub(super) fn delete_lines(&mut self, nlines: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_lines(&mut view.cursor, nlines);
        }
        self.snap_to_cursor();
    }

    pub(super) fn delete_lines_up(&mut self, nlines: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_lines_up(&mut view.cursor, nlines);
        }
        self.snap_to_cursor();
    }

    pub(super) fn delete_lines_down(&mut self, nlines: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_lines_down(&mut view.cursor, nlines);
        }
        self.snap_to_cursor();
    }

    pub(super) fn delete_to_line(&mut self, linum: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_to_line(&mut view.cursor, linum);
        }
        self.snap_to_cursor();
    }

    pub(super) fn delete_to_last_line(&mut self) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_to_last_line(&mut view.cursor);
        }
        self.snap_to_cursor();
    }

    pub(super) fn delete_to_line_start(&mut self) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_to_line_start(&mut view.cursor);
        }
        self.snap_to_cursor();
    }

    pub(super) fn delete_to_line_end(&mut self) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_to_line_end(&mut view.cursor);
        }
        self.snap_to_cursor();
    }

    pub(super) fn insert_char(&mut self, c: char) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.insert_char(&mut view.cursor, c);
        }
        self.snap_to_cursor();
    }

    pub(super) fn insert_str(&mut self, s: &str) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.insert_str(&mut view.cursor, s);
        }
        self.snap_to_cursor();
    }

    pub(super) fn scroll(&mut self, amts: (i32, i32)) {
        let view = &mut self.views[self.cur_view_idx];
        let buffer = &*view.buffer.borrow();
        let cursor_linum = view.cursor.line_num();
        let (shaped_linums, shaped_text) = buffer.shaped_data(self.dpi).unwrap();

        let (x, mut y) = (view.xbase as i32 + amts.0, view.ybase as i32 + amts.1);

        view.xbase = if x < 0 {
            0
        } else {
            // TODO Get max width of lines and make sure x is bounded such that the longest line
            // fills the screen?
            x as u32
        };

        let mut iter = LinumTextIter::new(
            shaped_linums,
            shaped_text,
            view.start_line,
            cursor_linum,
            view.line_numbers,
            view.relative_number,
        );

        view.ybase = if y < 0 {
            while let Some((_, _, height, _, _)) = iter.prev() {
                y += height as i32;
                view.start_line -= 1;
                if y >= 0 {
                    break;
                }
            }
            if y < 0 {
                0
            } else {
                y as u32
            }
        } else {
            while let Some((_, _, height, _, _)) = iter.next() {
                if y < height as i32 {
                    break;
                }
                y -= height as i32;
                view.start_line += 1;
            }
            if view.start_line + 1 >= shaped_text.len() {
                view.start_line = shaped_text.len();
                if view.start_line > 0 {
                    view.start_line -= 1;
                }
                y = 0;
            }
            y as u32
        };
    }

    pub(super) fn set_rect(&mut self, rect: Rect<u32, PixelSize>) {
        self.rect = rect;
        self.snap_to_cursor();
    }

    pub(super) fn draw(&mut self, actx: &mut ActiveRenderCtx) {
        let view = &mut self.views[self.cur_view_idx];
        let start_line = view.start_line;
        let cursor_linum = view.cursor.line_num();
        let buffer = &*view.buffer.borrow();
        let font_core = &mut *self.font_core.borrow_mut();
        let (shaped_linums, shaped_text) = buffer.shaped_data(self.dpi).unwrap();

        let gutter_width = if view.line_numbers || view.relative_number {
            shaped_linums[shaped_linums.len() - 1].metrics.width + self.gutter_padding * 2
        } else {
            self.gutter_padding * 2
        };

        let mut textview_rect = self.rect.cast();
        textview_rect.origin.x += gutter_width as i32;
        textview_rect.size.width -= gutter_width as i32;

        let mut pos = point2(-(view.xbase as i32), -(view.ybase as i32));
        {
            let mut linum = start_line;
            let mut ctx = actx.get_widget_context(textview_rect, self.background_color);
            for (ascender, _, height, line, _) in LinumTextIter::new(
                shaped_linums,
                shaped_text,
                start_line,
                cursor_linum,
                view.line_numbers,
                view.relative_number,
            ) {
                if pos.y >= textview_rect.size.height {
                    break;
                }
                let height = height as i32;
                let mut baseline = pos;
                baseline.y += ascender;
                let cursor = if linum == cursor_linum {
                    Some((
                        view.cursor.line_gidx(),
                        self.cursor_style,
                        self.cursor_color,
                    ))
                } else {
                    None
                };
                line.draw(&mut ctx, ascender, height, baseline, font_core, cursor);
                pos.y += height;
                linum += 1;
            }
        }

        let rect = Rect::new(self.rect.origin, size2(gutter_width, self.rect.size.height)).cast();
        if view.xbase > 0 {
            let vec = point2(3, 0).to_vector();
            actx.draw_shadow(rect.translate(vec));
        }

        pos = point2(
            (gutter_width - self.gutter_padding) as i32,
            -(view.ybase as i32),
        );
        {
            let mut linum = start_line;
            let mut ctx = actx.get_widget_context(rect, self.gutter_background_color);
            if view.line_numbers || view.relative_number {
                for (ascender, _, height, _, gline) in LinumTextIter::new(
                    shaped_linums,
                    shaped_text,
                    start_line,
                    cursor_linum,
                    view.line_numbers,
                    view.relative_number,
                ) {
                    if pos.y >= textview_rect.size.height {
                        break;
                    }
                    let gline = gline.unwrap();
                    let height = height as i32;
                    let mut baseline = pos;
                    baseline.y += ascender;
                    baseline.x -= gline.metrics.width as i32;
                    if view.line_numbers && view.relative_number && linum == cursor_linum {
                        baseline.x = self.gutter_padding as i32;
                    }
                    gline.draw(&mut ctx, ascender, height, baseline, font_core, None);
                    pos.y += height;
                    linum += 1;
                }
            }
        }
    }

    pub(super) fn set_line_numbers(&mut self, val: bool) {
        let view = &mut self.views[self.cur_view_idx];
        view.line_numbers = val;
    }

    pub(super) fn toggle_line_numbers(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        view.line_numbers = !view.line_numbers;
    }

    pub(super) fn set_relative_number(&mut self, val: bool) {
        let view = &mut self.views[self.cur_view_idx];
        view.relative_number = val;
    }

    pub(super) fn toggle_relative_number(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        view.relative_number = !view.relative_number;
    }

    fn snap_to_cursor(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let buffer = &*view.buffer.borrow();
        let cursor_linum = view.cursor.line_num();
        let (shaped_linums, shaped_text) = buffer.shaped_data(self.dpi).unwrap();

        let gutter_width = if view.line_numbers || view.relative_number {
            shaped_linums[shaped_linums.len() - 1].metrics.width + self.gutter_padding * 2
        } else {
            self.gutter_padding * 2
        };

        // Snap to y
        if cursor_linum <= view.start_line {
            view.start_line = cursor_linum;
            view.ybase = 0;
        } else {
            let mut total_height = 0;
            let mut linum = cursor_linum;
            let mut iter = LinumTextIter::new(
                shaped_linums,
                shaped_text,
                cursor_linum + 1,
                cursor_linum,
                view.line_numbers,
                view.relative_number,
            );
            while let Some((_, _, height, _, _)) = iter.prev() {
                total_height += height;
                if total_height >= self.rect.size.height {
                    view.ybase = total_height - self.rect.size.height;
                    view.start_line = linum;
                    break;
                }
                if linum == view.start_line {
                    break;
                }
                linum -= 1;
            }
        }

        // Snap to X
        let gidx = view.cursor.line_gidx();
        let line = &shaped_text[cursor_linum];
        let mut grapheme = 0;
        let mut cursor_x = 0;
        let width = self.rect.size.width - gutter_width;
        for span in &line.spans {
            for cluster in span.clusters() {
                if grapheme > gidx || grapheme + cluster.num_graphemes <= gidx {
                    for gi in cluster.glyph_infos {
                        cursor_x += gi.advance.width;
                    }
                    grapheme += cluster.num_graphemes;
                    continue;
                }
                let diff = gidx - grapheme;
                let num_glyphs = cluster.glyph_infos.len();
                let glyphs_per_grapheme = num_glyphs / cluster.num_graphemes;
                let start = diff * glyphs_per_grapheme;
                let end = start + glyphs_per_grapheme;
                for i in 0..start {
                    cursor_x += cluster.glyph_infos[i].advance.width;
                }
                let mut cursor_width = 0;
                for i in start..end {
                    cursor_width += cluster.glyph_infos[i].advance.width;
                }
                let cursor_x = if cursor_x < 0 { 0 } else { cursor_x as u32 };
                let cursor_width = if cursor_width < 0 {
                    0
                } else {
                    cursor_width as u32
                };
                if cursor_x < view.xbase {
                    view.xbase = cursor_x;
                } else if cursor_x + cursor_width > view.xbase + width {
                    view.xbase = cursor_x + cursor_width - width;
                }
                return;
            }
        }
    }
}

struct LinumTextIter<'a> {
    linums: &'a [ShapedTextLine],
    textlines: &'a [ShapedTextLine],
    i: usize,
    cursor_line: usize,
    numbers: bool,
    rela: bool,
}

impl<'a> LinumTextIter<'a> {
    fn new(
        linums: &'a [ShapedTextLine],
        textlines: &'a [ShapedTextLine],
        start_line: usize,
        cursor_line: usize,
        line_numbers: bool,
        relative_line_numbers: bool,
    ) -> LinumTextIter<'a> {
        LinumTextIter {
            linums: linums,
            textlines: textlines,
            i: start_line,
            cursor_line: cursor_line,
            numbers: line_numbers,
            rela: relative_line_numbers,
        }
    }

    fn prev(
        &mut self,
    ) -> Option<(
        i32,
        i32,
        u32,
        &'a ShapedTextLine,
        Option<&'a ShapedTextLine>,
    )> {
        if self.i == 0 {
            None
        } else {
            self.i -= 1;
            let tline = &self.textlines[self.i];
            let mut height = tline.metrics.height;
            let mut ascender = tline.metrics.ascender;
            let mut descender = tline.metrics.descender;
            let lline = if self.rela {
                let idx = if self.numbers && self.i == self.cursor_line {
                    self.i + 1
                } else if self.cursor_line > self.i {
                    self.cursor_line - self.i
                } else {
                    self.i - self.cursor_line
                };
                let lline = &self.linums[idx];
                height = max(height, lline.metrics.height);
                ascender = max(ascender, lline.metrics.ascender);
                descender = min(ascender, lline.metrics.descender);
                Some(lline)
            } else if self.numbers {
                let lline = &self.linums[self.i + 1];
                height = max(height, lline.metrics.height);
                ascender = max(ascender, lline.metrics.ascender);
                descender = min(ascender, lline.metrics.descender);
                Some(lline)
            } else {
                None
            };
            Some((ascender, descender, height, tline, lline))
        }
    }
}

impl<'a> Iterator for LinumTextIter<'a> {
    // ascender, descender, height, textline, linum
    type Item = (
        i32,
        i32,
        u32,
        &'a ShapedTextLine,
        Option<&'a ShapedTextLine>,
    );

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.textlines.len() {
            None
        } else {
            let tline = &self.textlines[self.i];
            let mut height = tline.metrics.height;
            let mut ascender = tline.metrics.ascender;
            let mut descender = tline.metrics.descender;
            let lline = if self.rela {
                let idx = if self.numbers && self.i == self.cursor_line {
                    self.i + 1
                } else if self.cursor_line > self.i {
                    self.cursor_line - self.i
                } else {
                    self.i - self.cursor_line
                };
                let lline = &self.linums[idx];
                height = max(height, lline.metrics.height);
                ascender = max(ascender, lline.metrics.ascender);
                descender = min(ascender, lline.metrics.descender);
                Some(lline)
            } else if self.numbers {
                let lline = &self.linums[self.i + 1];
                height = max(height, lline.metrics.height);
                ascender = max(ascender, lline.metrics.ascender);
                descender = min(ascender, lline.metrics.descender);
                Some(lline)
            } else {
                None
            };
            //println!("line: {}: height: {}", self.i, height);
            self.i += 1;
            Some((ascender, descender, height, tline, lline))
        }
    }
}
