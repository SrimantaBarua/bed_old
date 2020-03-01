// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use euclid::{point2, Rect, Size2D};

use crate::textbuffer::Buffer;
use crate::types::{Color, PixelSize, DPI};

use super::context::ActiveRenderCtx;
use super::font::{FaceKey, FontCore};
use super::text::{ShapedTextSpan, TextLine};

struct View {
    start_line: usize,
    buffer: Rc<RefCell<Buffer>>,
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
        let span_metrics = line.0[0].base_face_metrics(fixed_face, variable_face, font_core, dpi);
        let mut spans = Vec::new();
        let (mut ascender, mut descender, mut width) =
            (span_metrics.ascender, span_metrics.descender, 0);
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
}

pub(super) struct TextView {
    views: Vec<View>,
    cur_view_idx: usize,
    rect: Rect<u32, PixelSize>,
    background_color: Color,
    fixed_face: FaceKey,
    variable_face: FaceKey,
    lines: VecDeque<TextViewLine>,
    dpi: Size2D<u32, DPI>,
    font_core: Rc<RefCell<FontCore>>,
    xbase: u32,
    ybase: u32,
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
    ) -> TextView {
        let views = vec![View {
            start_line: 0,
            buffer: buffer,
        }];
        let mut textview = TextView {
            views: views,
            cur_view_idx: 0,
            rect: rect,
            background_color: background_color,
            fixed_face: fixed_face,
            variable_face: variable_face,
            lines: VecDeque::new(),
            font_core: font_core,
            dpi: dpi,
            xbase: 0,
            ybase: 0,
        };
        textview.refresh();
        textview
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
        let mut y = self.ybase as i32;
        y += amts.1;
        if y < 0 {
            // Scroll up
            {
                let font_core = &mut *self.font_core.borrow_mut();
                let buffer = &*view.buffer.borrow();
                let pos = buffer.get_pos_at_line(view.start_line);
                let mut iter = buffer.fmt_lines_from_pos(&pos);
                while let Some(line) = iter.prev() {
                    let fmtline = TextViewLine::from_textline(
                        line,
                        self.fixed_face,
                        self.variable_face,
                        font_core,
                        self.dpi,
                    );
                    y += fmtline.metrics.height as i32;
                    view.start_line -= 1;
                    self.lines.push_front(fmtline);
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
                if y < line.metrics.height as i32 {
                    self.lines.push_front(line);
                    found = true;
                    break;
                }
                view.start_line += 1;
                y -= line.metrics.height as i32;
            }
            if !found {
                let font_core = &mut *self.font_core.borrow_mut();
                let buffer = &*view.buffer.borrow();
                let len_lines = buffer.len_lines();
                if view.start_line < len_lines {
                    let pos = buffer.get_pos_at_line(view.start_line);
                    for line in buffer.fmt_lines_from_pos(&pos) {
                        let fmtline = TextViewLine::from_textline(
                            line,
                            self.fixed_face,
                            self.variable_face,
                            font_core,
                            self.dpi,
                        );
                        if y < fmtline.metrics.height as i32 {
                            self.lines.push_back(fmtline);
                            found = true;
                            break;
                        }
                        y -= fmtline.metrics.height as i32;
                        view.start_line += 1;
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

    pub(super) fn draw<'a>(&mut self, actx: &'a mut ActiveRenderCtx<'a>) {
        let mut ctx = actx.get_widget_context(self.rect.cast(), self.background_color);
        let mut pos = point2(-(self.xbase as i32), -(self.ybase as i32));

        let font_core = &mut *self.font_core.borrow_mut();

        for line in &self.lines {
            let mut pos_here = pos;
            pos_here.y += line.metrics.ascender;

            for span in &line.spans {
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

            pos.y += line.metrics.height as i32;
        }
    }

    pub(super) fn refresh(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let buffer = &*view.buffer.borrow();
        let pos = buffer.get_pos_at_line(view.start_line);
        view.start_line = pos.line_num();
        self.lines.clear();
        let font_core = &mut *self.font_core.borrow_mut();
        let mut height = 0;
        for line in buffer.fmt_lines_from_pos(&pos) {
            let fmtline = TextViewLine::from_textline(
                line,
                self.fixed_face,
                self.variable_face,
                font_core,
                self.dpi,
            );
            height += fmtline.metrics.height;
            self.lines.push_back(fmtline);
            if height >= self.rect.size.height + self.ybase {
                break;
            }
        }
    }

    fn trim_lines_at_end(&mut self) {
        let mut total_height = 0;
        for line in &self.lines {
            total_height += line.metrics.height;
        }
        while let Some(line) = self.lines.pop_back() {
            if total_height - line.metrics.height < self.rect.size.height {
                self.lines.push_back(line);
                break;
            }
            total_height -= line.metrics.height;
        }
    }

    fn fill_lines_at_end(&mut self) {
        let view = &mut self.views[self.cur_view_idx];
        let start_line = view.start_line + self.lines.len();
        let mut height = 0;
        for line in &self.lines {
            height += line.metrics.height;
        }
        let buffer = &*view.buffer.borrow();
        if start_line >= buffer.len_lines() {
            return;
        }
        let pos = buffer.get_pos_at_line(start_line);
        let font_core = &mut *self.font_core.borrow_mut();
        for line in buffer.fmt_lines_from_pos(&pos) {
            let fmtline = TextViewLine::from_textline(
                line,
                self.fixed_face,
                self.variable_face,
                font_core,
                self.dpi,
            );
            height += fmtline.metrics.height;
            self.lines.push_back(fmtline);
            if height >= self.rect.size.height + self.ybase {
                break;
            }
        }
    }
}
