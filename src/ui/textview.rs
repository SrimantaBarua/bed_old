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
    dpi: Size2D<u32, DPI>,
    font_core: Rc<RefCell<FontCore>>,
    xbase: u32,
    ybase: u32,
    line_numbers: bool,
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
            xbase: 0,
            ybase: 0,
            line_numbers: line_numbers,
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
            buffer: buffer,
            cursor: cursor,
        });
        self.cur_view_idx += 1;
        self.xbase = 0;
        self.ybase = 0;
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
        self.ybase = 0;
        let mut buf = String::new();
        self.lines.clear();
        let view = &mut self.views[self.cur_view_idx];
        {
            let font_core = &mut *self.font_core.borrow_mut();
            let buffer = &mut *view.buffer.borrow_mut();
            let pos = buffer.get_pos_at_line(view.start_line);
            let mut linum = view.start_line;
            let mut iter = buffer.fmt_lines_from_pos(&pos);
            let mut height = 0;
            while let Some(line) = iter.prev(&mut buf) {
                let fmtline = ShapedTextLine::from_textline(
                    line,
                    self.fixed_face,
                    self.variable_face,
                    font_core,
                    self.dpi,
                );

                buf.clear();
                write!(&mut buf, "{}", linum).unwrap();
                let gutterline = ShapedTextLine::from_textstr(
                    textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                    self.fixed_face,
                    self.variable_face,
                    font_core,
                    self.dpi,
                );

                height += max(fmtline.metrics.height, gutterline.metrics.height);
                view.start_line -= 1;
                linum -= 1;
                self.lines.push_front(fmtline);
                self.gutter.push_front(gutterline);

                if height >= self.rect.size.height {
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
        // Scroll x
        let mut x = self.xbase as i32;
        x += amts.0;
        self.xbase = if x < 0 {
            0
        } else {
            // TODO Get max width of lines and make sure x is bounded such that the longest line
            // fills the screen?
            x as u32
        };
        // Scroll y
        let view = &mut self.views[self.cur_view_idx];
        let mut buf = String::new();
        let mut y = self.ybase as i32;
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

                    buf.clear();
                    write!(&mut buf, "{}", linum).unwrap();
                    let gutterline = ShapedTextLine::from_textstr(
                        textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                        self.fixed_face,
                        self.variable_face,
                        font_core,
                        self.dpi,
                    );

                    y += max(fmtline.metrics.height, gutterline.metrics.height) as i32;
                    view.start_line -= 1;
                    linum -= 1;
                    self.lines.push_front(fmtline);
                    self.gutter.push_front(gutterline);

                    if y >= 0 {
                        break;
                    }
                }
            }
            if y < 0 {
                y = 0;
            }
            self.ybase = y as u32;
            self.trim_lines_at_end();
        } else if amts.1 > 0 {
            // Scroll down
            let mut found = false;
            while let Some(line) = self.lines.pop_front() {
                let gutterline = self.gutter.pop_front().unwrap();
                let height = max(line.metrics.height, gutterline.metrics.height) as i32;
                if y < height {
                    self.lines.push_front(line);
                    self.gutter.push_front(gutterline);
                    found = true;
                    break;
                }
                view.start_line += 1;
                y -= height;
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

                        buf.clear();
                        write!(&mut buf, "{}", linum).unwrap();
                        let gutterline = ShapedTextLine::from_textstr(
                            textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                            self.fixed_face,
                            self.variable_face,
                            font_core,
                            self.dpi,
                        );

                        let height = max(fmtline.metrics.height, gutterline.metrics.height) as i32;
                        if y < height {
                            self.lines.push_back(fmtline);
                            self.gutter.push_back(gutterline);
                            found = true;
                            break;
                        }
                        y -= height;
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
            self.ybase = y as u32;
            self.fill_lines_at_end();
            self.trim_lines_at_start();
        } else {
            self.ybase = y as u32;
        }
    }

    pub(super) fn set_rect(&mut self, rect: Rect<u32, PixelSize>) {
        self.rect = rect;
        self.refresh();
    }

    pub(super) fn draw(&mut self, actx: &mut ActiveRenderCtx) {
        let mut textview_rect = self.rect.cast();
        let font_core = &mut *self.font_core.borrow_mut();

        if self.line_numbers {
            textview_rect.origin.x += self.gutter_width as i32;
            textview_rect.size.width -= self.gutter_width as i32;
        }

        {
            let mut ctx = actx.get_widget_context(textview_rect, self.background_color);
            let mut pos = point2(-(self.xbase as i32), -(self.ybase as i32));

            let view = &mut self.views[self.cur_view_idx];

            for i in 0..self.lines.len() {
                let linum = view.start_line + i;
                let line = &self.lines[i];
                let mut baseline = pos;
                let ascender = max(line.metrics.ascender, self.gutter[i].metrics.ascender);
                baseline.y += ascender;
                let height = max(line.metrics.height, self.gutter[i].metrics.height) as i32;
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

        if self.line_numbers {
            let rect = Rect::new(
                self.rect.origin,
                size2(self.gutter_width, self.rect.size.height),
            )
            .cast();

            if self.xbase > 0 {
                let vec = point2(5, 0).to_vector();
                actx.draw_shadow(rect.translate(vec));
            }

            let mut ctx = actx.get_widget_context(rect, self.gutter_background_color);
            let mut pos = point2(
                (self.gutter_width - self.gutter_padding) as i32,
                -(self.ybase as i32),
            );

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
        let (mut height, mut linum) = (0, view.start_line + 1);

        // Max gutter width, to accomodate last line number of buffer
        let mut buf = format!("{}", buffer.len_lines());
        let line = ShapedTextLine::from_textstr(
            textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
            self.fixed_face,
            self.variable_face,
            font_core,
            self.dpi,
        );
        self.gutter_width = line.metrics.width + self.gutter_padding * 2;

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
            buf.clear();
            write!(&mut buf, "{}", linum).unwrap();
            let gutterline = ShapedTextLine::from_textstr(
                textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                self.fixed_face,
                self.variable_face,
                font_core,
                self.dpi,
            );

            height += max(fmtline.metrics.height, gutterline.metrics.height);
            linum += 1;
            self.lines.push_back(fmtline);
            self.gutter.push_back(gutterline);
            if height >= self.rect.size.height + self.ybase {
                break;
            }
        }
    }

    pub(super) fn set_line_numbers(&mut self, val: bool) {
        self.line_numbers = val;
    }

    pub(super) fn toggle_line_numbers(&mut self) {
        self.line_numbers = !self.line_numbers;
    }

    fn trim_lines_at_end(&mut self) {
        let mut total_height = 0;
        for i in 0..self.lines.len() {
            total_height += max(self.lines[i].metrics.height, self.gutter[i].metrics.height);
        }
        while let Some(line) = self.lines.pop_back() {
            let gutterline = self.gutter.pop_back().unwrap();
            let height = max(line.metrics.height, gutterline.metrics.height);
            if total_height - height < self.rect.size.height + self.ybase {
                self.lines.push_back(line);
                self.gutter.push_back(gutterline);
                break;
            }
            total_height -= height;
        }
    }

    fn trim_lines_at_start(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let mut total_height = 0;
        for i in 0..self.lines.len() {
            total_height += max(self.lines[i].metrics.height, self.gutter[i].metrics.height);
        }
        while let Some(line) = self.lines.pop_front() {
            let gutterline = self.gutter.pop_front().unwrap();
            let height = max(line.metrics.height, gutterline.metrics.height);
            if total_height - height < self.rect.size.height + self.ybase {
                self.lines.push_front(line);
                self.gutter.push_front(gutterline);
                break;
            }
            view.start_line += 1;
            total_height -= height;
        }
    }

    fn fill_lines_at_end(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let start_line = view.start_line + self.lines.len();
        let mut height = 0;
        for i in 0..self.lines.len() {
            height += max(self.lines[i].metrics.height, self.gutter[i].metrics.height);
        }
        let buffer = &mut *view.buffer.borrow_mut();
        if start_line >= buffer.len_lines() || height >= self.rect.size.height + self.ybase {
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

            buf.clear();
            write!(&mut buf, "{}", linum).unwrap();
            let gutterline = ShapedTextLine::from_textstr(
                textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                self.fixed_face,
                self.variable_face,
                font_core,
                self.dpi,
            );

            height += max(fmtline.metrics.height, gutterline.metrics.height);
            linum += 1;
            self.lines.push_back(fmtline);
            self.gutter.push_back(gutterline);

            if height >= self.rect.size.height + self.ybase {
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
            lines_height += max(self.lines[i].metrics.height, self.gutter[i].metrics.height);
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

                    buf.clear();
                    write!(&mut buf, "{}", linum).unwrap();
                    let gutterline = ShapedTextLine::from_textstr(
                        textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                        self.fixed_face,
                        self.variable_face,
                        font_core,
                        self.dpi,
                    );

                    view.start_line -= 1;
                    linum -= 1;
                    self.lines.push_front(fmtline);
                    self.gutter.push_front(gutterline);

                    if view.start_line == cursor_linum {
                        break;
                    }
                }
            }
            self.ybase = 0;
            self.trim_lines_at_end();
        } else if cursor_linum == view.start_line && self.ybase != 0 {
            // If cursor is at start line but y is not zero
            self.ybase = 0;
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

                    buf.clear();
                    write!(&mut buf, "{}", linum).unwrap();
                    let gutterline = ShapedTextLine::from_textstr(
                        textstr(&buf, self.gutter_textsize, self.gutter_foreground_color),
                        self.fixed_face,
                        self.variable_face,
                        font_core,
                        self.dpi,
                    );

                    lines_height += max(fmtline.metrics.height, gutterline.metrics.height);
                    linum += 1;
                    view.start_line += 1;
                    diff -= 1;
                    self.lines.push_back(fmtline);
                    self.gutter.push_back(gutterline);

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
                let height = max(self.lines[0].metrics.height, self.gutter[0].metrics.height);
                if lines_height - height < self.rect.size.height + self.ybase {
                    break;
                }
                lines_height -= height;
                self.lines.pop_front();
                self.gutter.pop_front();
            }
            if lines_height <= self.rect.size.height {
                self.ybase = 0;
            } else {
                self.ybase = lines_height - self.rect.size.height;
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
                if cursor_x < self.xbase {
                    self.xbase = cursor_x;
                } else if cursor_x + cursor_width > self.xbase + self.rect.size.width {
                    self.xbase = cursor_x + cursor_width - self.rect.size.width;
                }
                return;
            }
        }
    }
}

fn textstr(s: &str, size: TextSize, color: Color) -> TextSpan {
    TextSpan::new(s, size, TextStyle::default(), color, TextPitch::Fixed, None)
}
