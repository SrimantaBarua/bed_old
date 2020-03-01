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
        };
        textview.refresh();
        textview
    }

    pub(super) fn set_rect(&mut self, rect: Rect<u32, PixelSize>) {
        self.rect = rect;
        self.refresh();
    }

    pub(super) fn draw<'a>(&mut self, actx: &'a mut ActiveRenderCtx<'a>) {
        let mut ctx = actx.get_widget_context(self.rect.cast(), self.background_color);
        let mut pos = point2(0, 0);

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
        for line in buffer.fmt_lines_from_pos(&pos) {
            self.lines.push_back(TextViewLine::from_textline(
                line,
                self.fixed_face,
                self.variable_face,
                font_core,
                self.dpi,
            ));
            let nlines = self.lines.len();
            if self.lines[nlines - 1].metrics.height >= self.rect.size.height {
                break;
            }
        }
    }
}
