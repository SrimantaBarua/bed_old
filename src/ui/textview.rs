// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::cmp::max;
use std::collections::VecDeque;
use std::fmt::Write as FmtWrite;
use std::rc::Rc;

use euclid::{point2, size2, Rect, Size2D};

use crate::textbuffer::{Buffer, BufferCursor};
use crate::types::{Color, PixelSize, TextPitch, TextSize, TextStyle, DPI};

use super::context::ActiveRenderCtx;
use super::font::{FaceKey, FontCore};
use super::text::{ShapedTextSpan, TextCursorStyle, TextLine, TextSpan};

struct View {
    start_line: usize,
    buffer: Rc<RefCell<Buffer>>,
    main_cursor: BufferCursor,
    other_cursors: Vec<BufferCursor>,
}

struct TextViewLineMetrics {
    ascender: i32,
    descender: i32,
    height: u32,
    width: u32,
}

struct TextViewLine {
    metrics: TextViewLineMetrics,
    spans: Vec<ShapedTextSpan>,
}

impl TextViewLine {
    fn from_textline(
        line: TextLine,
        fixed_face: FaceKey,
        variable_face: FaceKey,
        font_core: &mut FontCore,
        dpi: Size2D<u32, DPI>,
    ) -> TextViewLine {
        assert!(line.0.len() > 0);
        let mut spans = Vec::new();
        let (mut ascender, mut descender, mut width) = (0, 0, 0);
        for span in line.0 {
            for shaped_span in span.shaped_spans(fixed_face, variable_face, font_core, dpi) {
                let span_metrics = &shaped_span.metrics;
                if span_metrics.ascender > ascender {
                    ascender = span_metrics.ascender;
                }
                if span_metrics.descender < descender {
                    descender = span_metrics.descender;
                }
                for gi in shaped_span.glyph_infos.iter() {
                    width += gi.advance.width;
                }
                spans.push(shaped_span);
            }
        }
        assert!(ascender > descender);
        let metrics = TextViewLineMetrics {
            ascender: ascender,
            descender: descender,
            height: (ascender - descender) as u32,
            width: if width < 0 { 0 } else { width as u32 },
        };
        TextViewLine {
            spans: spans,
            metrics: metrics,
        }
    }

    fn from_textstr(
        span: TextSpan,
        fixed_face: FaceKey,
        variable_face: FaceKey,
        font_core: &mut FontCore,
        dpi: Size2D<u32, DPI>,
    ) -> TextViewLine {
        let mut spans = Vec::new();
        let (mut ascender, mut descender, mut width) = (0, 0, 0);
        for shaped_span in span.shaped_spans(fixed_face, variable_face, font_core, dpi) {
            let span_metrics = &shaped_span.metrics;
            if span_metrics.ascender > ascender {
                ascender = span_metrics.ascender;
            }
            if span_metrics.descender < descender {
                descender = span_metrics.descender;
            }
            for gi in shaped_span.glyph_infos.iter() {
                width += gi.advance.width;
            }
            spans.push(shaped_span);
        }
        assert!(ascender > descender);
        let metrics = TextViewLineMetrics {
            ascender: ascender,
            descender: descender,
            height: (ascender - descender) as u32,
            width: if width < 0 { 0 } else { width as u32 },
        };
        TextViewLine {
            spans: spans,
            metrics: metrics,
        }
    }
}

pub(super) struct TextView {
    views: Vec<View>,
    cur_view_idx: usize,
    rect: Rect<u32, PixelSize>,
    background_color: Color,
    fixed_face: FaceKey,
    variable_face: FaceKey,
    lines: VecDeque<TextViewLine>,
    gutter: VecDeque<TextViewLine>,
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
        cursor_style: TextCursorStyle,
    ) -> TextView {
        let cursor = {
            let borrow = &mut *buffer.borrow_mut();
            let pos = borrow.get_pos_at_line(0);
            borrow.add_cursor_at_pos(&pos)
        };
        let views = vec![View {
            start_line: 0,
            buffer: buffer,
            main_cursor: cursor,
            other_cursors: Vec::new(),
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
            cursor_style: cursor_style,
        };
        textview.refresh();
        textview
    }

    pub(super) fn set_cursor_style(&mut self, style: TextCursorStyle) {
        self.cursor_style = style;
        let view = &mut self.views[self.cur_view_idx];
        let buffer = &mut *view.buffer.borrow_mut();
        buffer.move_cursor_left(&mut view.main_cursor, 1);
        for other in &mut view.other_cursors {
            buffer.move_cursor_left(other, 1);
        }
    }

    pub(super) fn move_cursor_down(&mut self, n: usize) {}

    pub(super) fn move_cursor_up(&mut self, n: usize) {}

    pub(super) fn move_cursor_left(&mut self, n: usize) {}

    pub(super) fn move_cursor_right(&mut self, n: usize) {}

    pub(super) fn move_cursor_start_of_line(&mut self) {}

    pub(super) fn move_cursor_end_of_line(&mut self) {}

    pub(super) fn page_up(&mut self) {}

    pub(super) fn page_down(&mut self) {}

    pub(super) fn go_to_line(&mut self, linum: usize) {}

    pub(super) fn go_to_last_line(&mut self) {}

    pub(super) fn delete_left(&mut self, n: usize) {}

    pub(super) fn delete_right(&mut self, n: usize) {}

    pub(super) fn delete_line(&mut self, n: usize) {}

    pub(super) fn delete_lines_up(&mut self, nlines: usize) {}

    pub(super) fn delete_lines_down(&mut self, nlines: usize) {}

    pub(super) fn delete_to_line(&mut self, linum: usize) {}

    pub(super) fn delete_to_last_line(&mut self) {}

    pub(super) fn delete_to_line_start(&mut self) {}

    pub(super) fn delete_to_line_end(&mut self) {}

    pub(super) fn insert_char(&mut self, c: char) {}

    pub(super) fn insert_str(&mut self, s: &str) {
        {
            let view = &mut self.views[self.cur_view_idx];
            let buffer = &mut *view.buffer.borrow_mut();
            buffer.insert_str(&mut view.main_cursor, s);
            for other in &mut view.other_cursors {
                buffer.insert_str(other, s);
            }
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
                    let fmtline = TextViewLine::from_textline(
                        line,
                        self.fixed_face,
                        self.variable_face,
                        font_core,
                        self.dpi,
                    );

                    buf.clear();
                    write!(&mut buf, "{}", linum).unwrap();
                    let gutterline = TextViewLine::from_textstr(
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
            self.trim_lines_at_end();
        } else {
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
                        let fmtline = TextViewLine::from_textline(
                            line,
                            self.fixed_face,
                            self.variable_face,
                            font_core,
                            self.dpi,
                        );

                        buf.clear();
                        write!(&mut buf, "{}", linum).unwrap();
                        let gutterline = TextViewLine::from_textstr(
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
            self.fill_lines_at_end();
        }
        self.ybase = y as u32;
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

            let mut ctx = actx.get_widget_context(
                Rect::new(
                    self.rect.origin,
                    size2(self.gutter_width, self.rect.size.height),
                )
                .cast(),
                self.gutter_background_color,
            );
            let mut pos = point2(
                (self.gutter_width - self.gutter_padding) as i32,
                -(self.ybase as i32),
            );

            for i in 0..self.gutter.len() {
                let gline = &self.gutter[i];
                let mut pos_here = pos;
                pos_here.y += max(self.lines[i].metrics.ascender, gline.metrics.ascender);
                pos_here.x -= gline.metrics.width as i32;

                for span in &gline.spans {
                    let (_, face) = font_core.get(span.face, span.style).unwrap();
                    for cluster in span.clusters() {
                        for gi in cluster.glyph_infos {
                            ctx.glyph(
                                pos_here + gi.offset,
                                span.face,
                                gi.gid,
                                span.size,
                                span.color,
                                span.style,
                                &mut face.raster,
                            );
                            pos_here.x += gi.advance.width;
                        }
                    }
                }

                pos.y += max(self.lines[i].metrics.height, gline.metrics.height) as i32;
            }
        }

        let mut ctx = actx.get_widget_context(textview_rect, self.background_color);
        let mut pos = point2(-(self.xbase as i32), -(self.ybase as i32));

        let view = &mut self.views[self.cur_view_idx];
        let main_cursor = &view.main_cursor;
        let mut cursor_idx = 0;
        while cursor_idx < view.other_cursors.len() {
            if view.other_cursors[cursor_idx].line_num() >= view.start_line {
                break;
            }
        }

        for i in 0..self.lines.len() {
            let line = &self.lines[i];
            let mut pos_here = pos;
            pos_here.y += max(line.metrics.ascender, self.gutter[i].metrics.ascender);

            let (mut grapheme, mut cursor_idx) = (0, 0);
            let height = max(line.metrics.height, self.gutter[i].metrics.height) as i32;

            for span in &line.spans {
                let underline_y = pos_here.y - span.metrics.underline_pos;
                let underline_thickness = span.metrics.underline_thickness;

                let (_, face) = font_core.get(span.face, span.style).unwrap();
                for cluster in span.clusters() {
                    let cursor_gidx = if i + view.start_line == main_cursor.line_num() {
                        let mgidx = main_cursor.line_gidx();
                        if cursor_idx < view.other_cursors.len() {
                            let other = &view.other_cursors[cursor_idx];
                            let (clinum, cgidx) = (other.line_num(), other.line_gidx());
                            if i + view.start_line == clinum && cgidx <= mgidx {
                                cursor_idx += 1;
                                Some(cgidx)
                            } else {
                                Some(mgidx)
                            }
                        } else {
                            Some(mgidx)
                        }
                    } else {
                        None
                    };
                    if let Some(gidx) = cursor_gidx {
                        if grapheme <= gidx && grapheme + cluster.num_graphemes > gidx {
                            let diff = gidx - grapheme;
                            let num_glyphs = cluster.glyph_infos.len();
                            let glyphs_per_grapheme = num_glyphs / cluster.num_graphemes;
                            let start = diff * glyphs_per_grapheme;
                            let end = start + glyphs_per_grapheme;
                            let mut cursor_x = pos_here.x;
                            for i in 0..start {
                                cursor_x += cluster.glyph_infos[i].advance.width;
                            }
                            let (cursor_y, cursor_size) = match self.cursor_style {
                                TextCursorStyle::Beam => (pos.y, size2(2, height)),
                                _ => {
                                    let mut width = 0;
                                    for i in start..end {
                                        width += cluster.glyph_infos[i].advance.width;
                                    }
                                    match self.cursor_style {
                                        TextCursorStyle::Block => (pos.y, size2(width, height)),
                                        TextCursorStyle::Underline => {
                                            (underline_y, size2(width, underline_thickness))
                                        }
                                        _ => unreachable!(),
                                    }
                                }
                            };
                            ctx.color_quad(
                                Rect::new(point2(cursor_x, cursor_y), cursor_size),
                                self.cursor_color,
                            );
                            cursor_idx += 1;
                        }
                    }

                    for gi in cluster.glyph_infos {
                        ctx.glyph(
                            pos_here + gi.offset,
                            span.face,
                            gi.gid,
                            span.size,
                            span.color,
                            span.style,
                            &mut face.raster,
                        );
                        pos_here.x += gi.advance.width;
                    }
                    grapheme += cluster.num_graphemes;
                }
            }

            pos.y += height;
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
        let line = TextViewLine::from_textstr(
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
            let fmtline = TextViewLine::from_textline(
                line,
                self.fixed_face,
                self.variable_face,
                font_core,
                self.dpi,
            );
            buf.clear();
            write!(&mut buf, "{}", linum).unwrap();
            let gutterline = TextViewLine::from_textstr(
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
            if total_height - height < self.rect.size.height {
                self.lines.push_back(line);
                self.gutter.push_back(gutterline);
                break;
            }
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
        if start_line >= buffer.len_lines() {
            return;
        }
        let pos = buffer.get_pos_at_line(start_line);
        let font_core = &mut *self.font_core.borrow_mut();

        let mut buf = String::new();
        let mut linum = start_line + 1;

        let mut iter = buffer.fmt_lines_from_pos(&pos);
        while let Some(line) = iter.next(&mut buf) {
            let fmtline = TextViewLine::from_textline(
                line,
                self.fixed_face,
                self.variable_face,
                font_core,
                self.dpi,
            );

            buf.clear();
            write!(&mut buf, "{}", linum).unwrap();
            let gutterline = TextViewLine::from_textstr(
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
        let cursor_linum = view.main_cursor.line_num();
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
                    let fmtline = TextViewLine::from_textline(
                        line,
                        self.fixed_face,
                        self.variable_face,
                        font_core,
                        self.dpi,
                    );

                    buf.clear();
                    write!(&mut buf, "{}", linum).unwrap();
                    let gutterline = TextViewLine::from_textstr(
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
                    let fmtline = TextViewLine::from_textline(
                        line,
                        self.fixed_face,
                        self.variable_face,
                        font_core,
                        self.dpi,
                    );

                    buf.clear();
                    write!(&mut buf, "{}", linum).unwrap();
                    let gutterline = TextViewLine::from_textstr(
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
                if lines_height - height < self.rect.size.height {
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
        let cursor_linum = view.main_cursor.line_num();
        let gidx = view.main_cursor.line_gidx();
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
