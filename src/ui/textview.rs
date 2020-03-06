// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::cmp::{max, min};
use std::collections::VecDeque;
use std::fmt::Write as FmtWrite;
use std::rc::Rc;

use euclid::{point2, size2, Rect, Size2D};

use crate::textbuffer::{Buffer, BufferCursor};
use crate::types::{Color, PixelSize, TextPitch, TextSize, TextStyle, DPI};

use super::context::ActiveRenderCtx;
use super::font::{FaceKey, FontCore};
use super::text::{ShapedTextLine, TextCursorStyle, TextSpan};

struct View {
    start_line: usize,
    xbase: u32,
    ybase: u32,
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
    lines: VecDeque<ShapedTextLine>,
    gutter: VecDeque<ShapedTextLine>,
    gutter_width: u32,
    gutter_padding: u32,
    gutter_textsize: TextSize,
    gutter_foreground_color: Color,
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
        gutter_foreground_color: Color,
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
            start_line: 0,
            xbase: 0,
            ybase: 0,
            line_numbers: line_numbers,
            relative_number: relative_number,
            buffer: buffer,
            cursor: cursor,
        }];
        let mut textview = TextView {
            views: views,
            cur_view_idx: 0,
            rect: rect,
            background_color: background_color,
            fixed_face: fixed_face,
            variable_face: variable_face,
            lines: VecDeque::new(),
            gutter: VecDeque::new(),
            gutter_width: 0,
            font_core: font_core,
            dpi: dpi,
            line_numbers: line_numbers,
            relative_number: relative_number,
            gutter_padding: gutter_padding,
            gutter_textsize: gutter_textsize,
            gutter_foreground_color: gutter_foreground_color,
            gutter_background_color: gutter_background_color,
            cursor_color: cursor_color,
            cursor_style: TextCursorStyle::Block,
        };
        textview.refresh();
        textview
    }

    pub(super) fn add_buffer(&mut self, buffer: Rc<RefCell<Buffer>>, view_id: usize) {
        let cursor = {
            let borrow = &mut *buffer.borrow_mut();
            let pos = borrow.get_pos_at_line(0);
            borrow.add_cursor_at_pos(view_id, &pos, false)
        };
        self.views.push(View {
            start_line: 0,
            xbase: 0,
            ybase: 0,
            line_numbers: self.line_numbers,
            relative_number: self.relative_number,
            buffer: buffer,
            cursor: cursor,
        });
        self.cur_view_idx += 1;
        self.refresh();
    }

    pub(super) fn prev_buffer(&mut self) {
        if self.cur_view_idx == 0 {
            self.cur_view_idx = self.views.len() - 1;
        } else {
            self.cur_view_idx -= 1;
        }
        self.refresh();
    }

    pub(super) fn next_buffer(&mut self) {
        self.cur_view_idx = (self.cur_view_idx + 1) % self.views.len();
        self.refresh();
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
        let view = &self.views[self.cur_view_idx];
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
        point.0 += view.xbase as i32 - self.gutter_width as i32;
        point.1 += view.ybase as i32;
        let mut total_height = 0;
        let mut linum = 0;
        for i in 0..self.lines.len() {
            let mut height = self.lines[i].metrics.height as i32;
            if view.line_numbers {
                height = max(height, self.gutter[i].metrics.height as i32);
            }
            total_height += height;
            if total_height >= point.1 {
                break;
            }
            linum += 1;
        }
        let mut x = 0;
        let mut gidx = 0;
        'outer: for span in &self.lines[linum].spans {
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
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.move_cursor_to_linum_gidx(
                &mut view.cursor,
                view.start_line + linum,
                gidx as usize,
            );
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
        let mut buf = String::new();
        self.lines.clear();
        let view = &mut self.views[self.cur_view_idx];
        view.ybase = 0;
        {
            let font_core = &mut *self.font_core.borrow_mut();
            let buffer = &mut *view.buffer.borrow_mut();
            let pos = buffer.get_pos_at_line(view.start_line);
            let mut linum = view.start_line;
            let mut iter = buffer.fmt_lines_from_pos(&pos);
            let mut total_height = 0;
            while let Some(line) = iter.prev(&mut buf) {
                let fmtline = ShapedTextLine::from_textline(
                    line,
                    self.fixed_face,
                    self.variable_face,
                    font_core,
                    self.dpi,
                );
                let mut height = fmtline.metrics.height;
                self.lines.push_front(fmtline);

                if view.line_numbers {
                    buf.clear();
                    write!(&mut buf, "{}", linum).unwrap();
                    let gutterline = ShapedTextLine::from_textstr(
                        textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                        self.fixed_face,
                        self.variable_face,
                        font_core,
                        self.dpi,
                    );
                    height = max(height, gutterline.metrics.height);
                    self.gutter.push_front(gutterline);
                }

                total_height += height;
                view.start_line -= 1;
                linum -= 1;

                if total_height >= self.rect.size.height {
                    break;
                }
            }

            let nlines = self.lines.len();
            if nlines < 2 {
                buffer.move_cursor_to_line(&mut view.cursor, view.start_line);
            } else if view.cursor.line_num() > view.start_line + nlines - 2 {
                buffer.move_cursor_to_line(&mut view.cursor, view.start_line + nlines - 2);
            }
        }
        self.refresh();
    }

    pub(super) fn page_down(&mut self) {
        let mut nlines = self.lines.len();
        if nlines > 0 {
            nlines -= 1;
        }
        let view = &mut self.views[self.cur_view_idx];
        {
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.move_cursor_down(&mut view.cursor, nlines);
        }
        view.start_line = view.cursor.line_num();
        self.refresh();
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
        self.refresh();
        self.snap_to_cursor();
    }

    pub(super) fn delete_right(&mut self, n: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_right(&mut view.cursor, n);
        }
        self.refresh();
        self.snap_to_cursor();
    }

    pub(super) fn delete_lines(&mut self, nlines: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_lines(&mut view.cursor, nlines);
        }
        self.refresh();
        self.snap_to_cursor();
    }

    pub(super) fn delete_lines_up(&mut self, nlines: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_lines_up(&mut view.cursor, nlines);
        }
        self.refresh();
        self.snap_to_cursor();
    }

    pub(super) fn delete_lines_down(&mut self, nlines: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_lines_down(&mut view.cursor, nlines);
        }
        self.refresh();
        self.snap_to_cursor();
    }

    pub(super) fn delete_to_line(&mut self, linum: usize) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_to_line(&mut view.cursor, linum);
        }
        self.refresh();
        self.snap_to_cursor();
    }

    pub(super) fn delete_to_last_line(&mut self) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_to_last_line(&mut view.cursor);
        }
        self.refresh();
        self.snap_to_cursor();
    }

    pub(super) fn delete_to_line_start(&mut self) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_to_line_start(&mut view.cursor);
        }
        self.refresh();
        self.snap_to_cursor();
    }

    pub(super) fn delete_to_line_end(&mut self) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.delete_to_line_end(&mut view.cursor);
        }
        self.refresh();
        self.snap_to_cursor();
    }

    pub(super) fn insert_char(&mut self, c: char) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.insert_char(&mut view.cursor, c);
        }
        self.refresh();
        self.snap_to_cursor();
    }

    pub(super) fn insert_str(&mut self, s: &str) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.insert_str(&mut view.cursor, s);
        }
        self.refresh();
        self.snap_to_cursor();
    }

    pub(super) fn scroll(&mut self, amts: (i32, i32)) {
        let view = &mut self.views[self.cur_view_idx];
        // Scroll x
        let mut x = view.xbase as i32;
        x += amts.0;
        view.xbase = if x < 0 {
            0
        } else {
            // TODO Get max width of lines and make sure x is bounded such that the longest line
            // fills the screen?
            x as u32
        };
        // Scroll y
        let mut buf = String::new();
        let mut y = view.ybase as i32;
        y += amts.1;
        if y < 0 {
            // Scroll up
            {
                let font_core = &mut *self.font_core.borrow_mut();
                let buffer = &mut *view.buffer.borrow_mut();
                let pos = buffer.get_pos_at_line(view.start_line);
                let mut linum = view.start_line;
                let mut iter = buffer.fmt_lines_from_pos(&pos);
                while let Some(line) = iter.prev(&mut buf) {
                    let fmtline = ShapedTextLine::from_textline(
                        line,
                        self.fixed_face,
                        self.variable_face,
                        font_core,
                        self.dpi,
                    );
                    let mut height = fmtline.metrics.height;
                    self.lines.push_front(fmtline);

                    if view.line_numbers {
                        buf.clear();
                        write!(&mut buf, "{}", linum).unwrap();
                        let gutterline = ShapedTextLine::from_textstr(
                            textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                            self.fixed_face,
                            self.variable_face,
                            font_core,
                            self.dpi,
                        );
                        height = max(height, gutterline.metrics.height);
                        self.gutter.push_front(gutterline);
                    }

                    y += height as i32;
                    view.start_line -= 1;
                    linum -= 1;

                    if y >= 0 {
                        break;
                    }
                }
            }
            if y < 0 {
                y = 0;
            }
            view.ybase = y as u32;
            self.trim_lines_at_end();
        } else if amts.1 > 0 {
            // Scroll down
            let mut found = false;
            while let Some(line) = self.lines.pop_front() {
                if view.line_numbers {
                    let gutterline = self.gutter.pop_front().unwrap();
                    let height = max(line.metrics.height, gutterline.metrics.height) as i32;
                    if y < height {
                        self.lines.push_front(line);
                        self.gutter.push_front(gutterline);
                        found = true;
                        break;
                    }
                    y -= height;
                } else {
                    if y < line.metrics.height as i32 {
                        self.lines.push_front(line);
                        found = true;
                        break;
                    }
                    y -= line.metrics.height as i32;
                }
                view.start_line += 1;
            }
            if !found {
                let font_core = &mut *self.font_core.borrow_mut();
                let buffer = &mut *view.buffer.borrow_mut();
                let len_lines = buffer.len_lines();
                if view.start_line < len_lines {
                    let pos = buffer.get_pos_at_line(view.start_line);
                    let mut linum = view.start_line + 1;

                    let mut iter = buffer.fmt_lines_from_pos(&pos);
                    while let Some(line) = iter.next(&mut buf) {
                        let fmtline = ShapedTextLine::from_textline(
                            line,
                            self.fixed_face,
                            self.variable_face,
                            font_core,
                            self.dpi,
                        );

                        if view.line_numbers {
                            buf.clear();
                            write!(&mut buf, "{}", linum).unwrap();
                            let gutterline = ShapedTextLine::from_textstr(
                                textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                                self.fixed_face,
                                self.variable_face,
                                font_core,
                                self.dpi,
                            );
                            let height =
                                max(fmtline.metrics.height, gutterline.metrics.height) as i32;
                            if y < height {
                                self.lines.push_back(fmtline);
                                self.gutter.push_back(gutterline);
                                found = true;
                                break;
                            }
                            y -= height;
                        } else {
                            if y < fmtline.metrics.height as i32 {
                                self.lines.push_back(fmtline);
                                found = true;
                                break;
                            }
                            y -= fmtline.metrics.height as i32;
                        }

                        view.start_line += 1;
                        linum += 1;
                    }
                }
                if !found {
                    if len_lines > 0 {
                        view.start_line = len_lines - 1;
                    } else {
                        view.start_line = len_lines;
                    }
                    y = 0;
                }
            }
            view.ybase = y as u32;
            self.fill_lines_at_end();
            self.trim_lines_at_start();
        } else {
            view.ybase = y as u32;
        }
    }

    pub(super) fn set_rect(&mut self, rect: Rect<u32, PixelSize>) {
        self.rect = rect;
        self.refresh();
    }

    pub(super) fn draw(&mut self, actx: &mut ActiveRenderCtx) {
        let view = &self.views[self.cur_view_idx];
        let mut textview_rect = self.rect.cast();
        let font_core = &mut *self.font_core.borrow_mut();

        textview_rect.origin.x += self.gutter_width as i32;
        textview_rect.size.width -= self.gutter_width as i32;
        {
            let mut ctx = actx.get_widget_context(textview_rect, self.background_color);
            let mut pos = point2(-(view.xbase as i32), -(view.ybase as i32));

            for i in 0..self.lines.len() {
                let line = &self.lines[i];
                let mut baseline = pos;
                let mut ascender = line.metrics.ascender;
                let mut height = line.metrics.height as i32;
                if view.line_numbers {
                    ascender = max(ascender, self.gutter[i].metrics.ascender);
                    height = max(height, self.gutter[i].metrics.height as i32);
                }
                baseline.y += ascender;
                let cursor = if view.start_line + i == view.cursor.line_num() {
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
            }
        }

        let rect = Rect::new(
            self.rect.origin,
            size2(self.gutter_width, self.rect.size.height),
        )
        .cast();

        if view.xbase > 0 {
            let vec = point2(5, 0).to_vector();
            actx.draw_shadow(rect.translate(vec));
        }

        let mut ctx = actx.get_widget_context(rect, self.gutter_background_color);
        let mut pos = point2(
            (self.gutter_width - self.gutter_padding) as i32,
            -(view.ybase as i32),
        );

        if view.line_numbers {
            for i in 0..self.gutter.len() {
                let gline = &self.gutter[i];
                let mut baseline = pos;
                let ascender = max(self.lines[i].metrics.ascender, gline.metrics.ascender);
                baseline.y += ascender;
                baseline.x -= gline.metrics.width as i32;
                let height = max(self.lines[i].metrics.height, gline.metrics.height) as i32;
                gline.draw(&mut ctx, ascender, height, baseline, font_core, None);
                pos.y += height;
            }
        }
    }

    pub(super) fn refresh(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let buffer = &mut *view.buffer.borrow_mut();
        let pos = buffer.get_pos_at_line(view.start_line);
        view.start_line = pos.line_num();
        self.lines.clear();
        self.gutter.clear();
        let font_core = &mut *self.font_core.borrow_mut();
        let (mut total_height, mut linum) = (0, view.start_line + 1);

        // Max gutter width, to accomodate last line number of buffer
        let mut buf = format!("{}", buffer.len_lines());
        if view.line_numbers {
            let line = ShapedTextLine::from_textstr(
                textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                self.fixed_face,
                self.variable_face,
                font_core,
                self.dpi,
            );
            self.gutter_width = line.metrics.width + self.gutter_padding * 2;
        } else {
            self.gutter_width = self.gutter_padding;
        }

        // Fill lines and gutter
        let mut iter = buffer.fmt_lines_from_pos(&pos);
        while let Some(line) = iter.next(&mut buf) {
            let fmtline = ShapedTextLine::from_textline(
                line,
                self.fixed_face,
                self.variable_face,
                font_core,
                self.dpi,
            );
            let mut height = fmtline.metrics.height;
            self.lines.push_back(fmtline);

            if view.line_numbers {
                buf.clear();
                write!(&mut buf, "{}", linum).unwrap();
                let gutterline = ShapedTextLine::from_textstr(
                    textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                    self.fixed_face,
                    self.variable_face,
                    font_core,
                    self.dpi,
                );
                height = max(height, gutterline.metrics.height);
                self.gutter.push_back(gutterline);
            }

            linum += 1;
            total_height += height;
            if total_height >= self.rect.size.height + view.ybase {
                break;
            }
        }
    }

    pub(super) fn set_line_numbers(&mut self, val: bool) {
        let view = &mut self.views[self.cur_view_idx];
        view.line_numbers = val;
        self.refresh();
    }

    pub(super) fn toggle_line_numbers(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        view.line_numbers = !view.line_numbers;
        self.refresh();
    }

    fn trim_lines_at_end(&mut self) {
        let view = &self.views[self.cur_view_idx];
        let mut total_height = 0;
        for i in 0..self.lines.len() {
            let mut height = self.lines[i].metrics.height;
            if view.line_numbers {
                height = max(height, self.gutter[i].metrics.height);
            }
            total_height += height;
        }
        while let Some(line) = self.lines.pop_back() {
            if view.line_numbers {
                let gutterline = self.gutter.pop_back().unwrap();
                let height = max(line.metrics.height, gutterline.metrics.height);
                if total_height - height < self.rect.size.height + view.ybase {
                    self.lines.push_back(line);
                    self.gutter.push_back(gutterline);
                    break;
                }
                total_height -= height;
            } else {
                let height = line.metrics.height;
                if total_height - height < self.rect.size.height + view.ybase {
                    self.lines.push_back(line);
                    break;
                }
                total_height -= height;
            }
        }
    }

    fn trim_lines_at_start(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let mut total_height = 0;
        for i in 0..self.lines.len() {
            let mut height = self.lines[i].metrics.height;
            if view.line_numbers {
                height = max(height, self.gutter[i].metrics.height);
            }
            total_height += height;
        }
        while let Some(line) = self.lines.pop_front() {
            if view.line_numbers {
                let gutterline = self.gutter.pop_front().unwrap();
                let height = max(line.metrics.height, gutterline.metrics.height);
                if total_height - height < self.rect.size.height + view.ybase {
                    self.lines.push_front(line);
                    self.gutter.push_front(gutterline);
                    break;
                }
                total_height -= height;
            } else {
                let height = line.metrics.height;
                if total_height - height < self.rect.size.height + view.ybase {
                    self.lines.push_front(line);
                    break;
                }
                total_height -= height;
            }
            view.start_line += 1;
        }
    }

    fn fill_lines_at_end(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let start_line = view.start_line + self.lines.len();
        let mut total_height = 0;
        for i in 0..self.lines.len() {
            let mut height = self.lines[i].metrics.height;
            if view.line_numbers {
                height = max(height, self.gutter[i].metrics.height);
            }
            total_height += height;
        }
        let buffer = &mut *view.buffer.borrow_mut();
        if start_line >= buffer.len_lines() || total_height >= self.rect.size.height + view.ybase {
            return;
        }
        let pos = buffer.get_pos_at_line(start_line);
        let font_core = &mut *self.font_core.borrow_mut();

        let mut buf = String::new();
        let mut linum = start_line + 1;

        let mut iter = buffer.fmt_lines_from_pos(&pos);
        while let Some(line) = iter.next(&mut buf) {
            let fmtline = ShapedTextLine::from_textline(
                line,
                self.fixed_face,
                self.variable_face,
                font_core,
                self.dpi,
            );
            let mut height = fmtline.metrics.height;
            self.lines.push_back(fmtline);

            if view.line_numbers {
                buf.clear();
                write!(&mut buf, "{}", linum).unwrap();
                let gutterline = ShapedTextLine::from_textstr(
                    textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                    self.fixed_face,
                    self.variable_face,
                    font_core,
                    self.dpi,
                );
                height = max(height, gutterline.metrics.height);
                self.gutter.push_back(gutterline);
            }

            linum += 1;
            total_height += height;

            if total_height >= self.rect.size.height + view.ybase {
                break;
            }
        }
    }

    pub(super) fn snap_to_cursor(&mut self) {
        self.snap_to_y();
        self.snap_to_x();
    }

    fn snap_to_y(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let num_lines = self.lines.len();
        let mut buf = String::new();
        let mut lines_height = 0;
        let cursor_linum = view.cursor.line_num();
        for i in 0..self.lines.len() {
            let mut height = self.lines[i].metrics.height;
            if view.line_numbers {
                height = max(height, self.gutter[i].metrics.height);
            }
            lines_height += height;
        }
        if cursor_linum < view.start_line {
            // If cursor is before start line
            {
                let font_core = &mut *self.font_core.borrow_mut();
                let buffer = &mut *view.buffer.borrow_mut();
                let pos = buffer.get_pos_at_line(view.start_line);
                let mut linum = view.start_line;
                let mut iter = buffer.fmt_lines_from_pos(&pos);
                while let Some(line) = iter.prev(&mut buf) {
                    let fmtline = ShapedTextLine::from_textline(
                        line,
                        self.fixed_face,
                        self.variable_face,
                        font_core,
                        self.dpi,
                    );
                    self.lines.push_front(fmtline);

                    if view.line_numbers {
                        buf.clear();
                        write!(&mut buf, "{}", linum).unwrap();
                        let gutterline = ShapedTextLine::from_textstr(
                            textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                            self.fixed_face,
                            self.variable_face,
                            font_core,
                            self.dpi,
                        );
                        self.gutter.push_front(gutterline);
                    }

                    view.start_line -= 1;
                    linum -= 1;

                    if view.start_line == cursor_linum {
                        break;
                    }
                }
            }
            view.ybase = 0;
            self.trim_lines_at_end();
        } else if cursor_linum == view.start_line && view.ybase != 0 {
            // If cursor is at start line but y is not zero
            view.ybase = 0;
            self.trim_lines_at_end();
        } else if lines_height >= self.rect.size.height
            && cursor_linum >= view.start_line + num_lines
        {
            // If cursor is beyond last line
            {
                let mut diff = cursor_linum - (view.start_line + num_lines) + 1;
                let font_core = &mut *self.font_core.borrow_mut();
                let buffer = &mut *view.buffer.borrow_mut();
                let pos = buffer.get_pos_at_line(view.start_line + num_lines);
                let mut linum = view.start_line + num_lines + 1;
                let mut iter = buffer.fmt_lines_from_pos(&pos);
                while let Some(line) = iter.next(&mut buf) {
                    let fmtline = ShapedTextLine::from_textline(
                        line,
                        self.fixed_face,
                        self.variable_face,
                        font_core,
                        self.dpi,
                    );
                    let mut height = fmtline.metrics.height;
                    self.lines.push_back(fmtline);

                    if view.line_numbers {
                        buf.clear();
                        write!(&mut buf, "{}", linum).unwrap();
                        let gutterline = ShapedTextLine::from_textstr(
                            textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                            self.fixed_face,
                            self.variable_face,
                            font_core,
                            self.dpi,
                        );
                        height = max(height, gutterline.metrics.height);
                        self.gutter.push_back(gutterline);
                    }

                    linum += 1;
                    view.start_line += 1;
                    lines_height += height;
                    diff -= 1;

                    if diff == 0 {
                        break;
                    }
                }
            }
        }
        let view = &mut self.views[self.cur_view_idx];
        if num_lines != 0 && cursor_linum == view.start_line + num_lines - 1 {
            // If cursor is at last line
            loop {
                let mut height = self.lines[0].metrics.height;
                if view.line_numbers {
                    height = max(height, self.gutter[0].metrics.height);
                }
                if lines_height - height < self.rect.size.height + view.ybase {
                    break;
                }
                lines_height -= height;
                self.lines.pop_front();
                if view.line_numbers {
                    self.gutter.pop_front();
                }
            }
            if lines_height <= self.rect.size.height {
                view.ybase = 0;
            } else {
                view.ybase = lines_height - self.rect.size.height;
            }
        }
    }

    fn snap_to_x(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let cursor_linum = view.cursor.line_num();
        let gidx = view.cursor.line_gidx();
        let line = &self.lines[cursor_linum - view.start_line];
        let mut grapheme = 0;
        let mut cursor_x = 0;
        let width = self.rect.size.width - self.gutter_width;
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

fn textstr(s: &str, size: TextSize, color: Color) -> TextSpan {
    TextSpan::new(s, size, TextStyle::default(), color, TextPitch::Fixed, None)
}
