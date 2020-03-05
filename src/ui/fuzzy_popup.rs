// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::rc::Rc;

use euclid::{point2, size2, Rect, SideOffsets2D, Size2D};
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

use crate::types::{Color, PixelSize, TextPitch, TextSize, TextStyle, DPI};

use super::context::ActiveRenderCtx;
use super::font::{FaceKey, FontCore};
use super::text::{ShapedTextSpan, TextCursorStyle, TextLine, TextSpan};

#[derive(Default)]
struct FuzzyPopupLineMetrics {
    ascender: i32,
    descender: i32,
    height: u32,
    width: u32,
}

#[derive(Default)]
struct FuzzyPopupLine {
    metrics: FuzzyPopupLineMetrics,
    spans: Vec<ShapedTextSpan>,
}

impl FuzzyPopupLine {
    fn from_textstr(
        span: TextSpan,
        fixed_face: FaceKey,
        variable_face: FaceKey,
        font_core: &mut FontCore,
        dpi: Size2D<u32, DPI>,
    ) -> FuzzyPopupLine {
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
        let metrics = FuzzyPopupLineMetrics {
            ascender: ascender,
            descender: descender,
            height: (ascender - descender) as u32,
            width: if width < 0 { 0 } else { width as u32 },
        };
        FuzzyPopupLine {
            spans: spans,
            metrics: metrics,
        }
    }
}

pub(super) struct FuzzyPopup {
    window_size: Size2D<u32, PixelSize>,
    max_height_percentage: u32,
    width_percentage: u32,
    height: u32,
    edge_padding: u32,
    text_padding: u32,
    bottom_off: u32,
    background_color: Color,
    foreground_color: Color,
    selected_color: Color,
    cursor_color: Color,
    text_size: TextSize,
    face: FaceKey,
    input_line: FuzzyPopupLine,
    lines: Vec<FuzzyPopupLine>,
    dpi: Size2D<u32, DPI>,
    font_core: Rc<RefCell<FontCore>>,
    user_input: String,
    choices: Vec<String>,
    filtered: Vec<(usize, String)>,
    is_active: bool,
    cursor_bidx: usize,
    cursor_gidx: usize,
}

impl FuzzyPopup {
    pub(super) fn new(
        window_size: Size2D<u32, PixelSize>,
        max_height_percentage: u32,
        width_percentage: u32,
        edge_padding: u32,
        text_padding: u32,
        bottom_off: u32,
        background_color: Color,
        foreground_color: Color,
        selected_color: Color,
        cursor_color: Color,
        text_size: TextSize,
        face: FaceKey,
        font_core: Rc<RefCell<FontCore>>,
        dpi: Size2D<u32, DPI>,
    ) -> FuzzyPopup {
        let mut ret = FuzzyPopup {
            window_size: window_size,
            max_height_percentage: max_height_percentage,
            width_percentage: width_percentage,
            edge_padding: edge_padding,
            text_padding: text_padding,
            bottom_off: bottom_off,
            height: 0,
            background_color: background_color,
            foreground_color: foreground_color,
            selected_color: selected_color,
            cursor_color: cursor_color,
            text_size: text_size,
            face: face,
            input_line: FuzzyPopupLine::default(),
            lines: Vec::new(),
            dpi: dpi,
            font_core: font_core,
            user_input: String::new(),
            choices: Vec::new(),
            filtered: Vec::new(),
            is_active: false,
            cursor_bidx: 0,
            cursor_gidx: 0,
        };
        ret.refresh();
        ret
    }

    pub(super) fn draw(&mut self, actx: &mut ActiveRenderCtx) {
        let width = (self.window_size.width * self.width_percentage) / 100;
        let lpad = (self.window_size.width - width) / 2;
        let origin = point2(
            lpad,
            self.window_size.height - self.height - self.bottom_off,
        );
        let size = size2(width, self.height);
        let side_offsets = SideOffsets2D::new(
            self.edge_padding,
            self.edge_padding,
            self.edge_padding,
            self.edge_padding,
        );
        let rect = Rect::new(origin, size);
        let inner_rect = rect.inner_rect(side_offsets);

        {
            let vec = point2(5, 5).to_vector();
            actx.draw_shadow(rect.translate(vec).cast());
            let _ctx = actx.get_widget_context(rect.cast(), self.background_color);
        }

        let font_core = &mut *self.font_core.borrow_mut();
        {
            let mut ctx = actx.get_widget_context(inner_rect.cast(), self.background_color);
            let mut pos = point2(0, inner_rect.size.height as i32);
            pos.y -= self.input_line.metrics.height as i32;

            // Draw input line
            let mut pos_here = pos;
            pos_here.y += self.input_line.metrics.ascender;
            let mut grapheme = 0;
            for span in &self.input_line.spans {
                let (_, face) = font_core.get(span.face, span.style).unwrap();
                for cluster in span.clusters() {
                    let num_glyphs = cluster.glyph_infos.len();
                    let glyphs_per_grapheme = num_glyphs / cluster.num_graphemes;
                    for j in (0..num_glyphs).step_by(glyphs_per_grapheme) {
                        if grapheme == self.cursor_gidx {
                            ctx.color_quad(
                                Rect::new(
                                    point2(pos_here.x, pos.y),
                                    size2(2, self.input_line.metrics.height).cast(),
                                ),
                                self.cursor_color,
                            );
                        }
                        for gi in &cluster.glyph_infos[j..(j + glyphs_per_grapheme)] {
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
                        grapheme += 1;
                    }
                }
            }
            if grapheme == self.cursor_gidx {
                ctx.color_quad(
                    Rect::new(
                        point2(pos_here.x, pos.y),
                        size2(2, self.input_line.metrics.height).cast(),
                    ),
                    self.cursor_color,
                );
            }

            // Draw selection lines
            if self.lines.len() > 0 {
                pos.y -= self.lines[0].metrics.height as i32;
                let rect = Rect::new(pos, size2(width, self.lines[0].metrics.height).cast());
                ctx.color_quad(rect, Color::new(0, 0, 0, 8));

                for i in 0..self.lines.len() {
                    let line = &self.lines[i];
                    if i > 0 {
                        pos.y -= line.metrics.height as i32;
                    }
                    let mut pos_here = pos;
                    pos_here.x += self.text_padding as i32;
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
                }
            }
        }
    }

    pub(super) fn fill_with<F>(&mut self, f: F)
    where
        F: Fn(&mut Vec<String>),
    {
        self.choices.clear();
        f(&mut self.choices);
        self.filter();
        self.refresh();
    }

    pub(super) fn is_active(&self) -> bool {
        self.is_active
    }

    pub(super) fn set_active(&mut self, val: bool) {
        self.is_active = val;
        self.user_input.clear();
        self.cursor_bidx = 0;
        self.cursor_gidx = 0;
    }

    pub(super) fn get_selection(&self, get_default_on_empty: bool) -> Option<String> {
        if self.filtered.len() > 0 {
            if get_default_on_empty || self.user_input.len() > 0 {
                Some(self.filtered[0].1.to_owned())
            } else {
                None
            }
        } else {
            None
        }
    }

    pub(super) fn insert(&mut self, c: char) {
        self.user_input.push(c);
        self.cursor_bidx = next_grapheme_boundary(&self.user_input, self.cursor_bidx);
        self.cursor_gidx = bidx_to_gidx(&self.user_input, self.cursor_bidx);
        self.filter();
        self.refresh();
    }

    pub(super) fn delete_left(&mut self) {
        if self.cursor_bidx == 0 {
            return;
        }
        let cur = self.cursor_bidx;
        self.cursor_bidx = 0;
        for (i, _) in self.user_input.char_indices() {
            if i >= cur {
                break;
            }
            self.cursor_bidx = i;
        }
        self.user_input.remove(self.cursor_bidx);
        if !is_grapheme_boundary(&self.user_input, self.cursor_bidx) {
            self.cursor_bidx = next_grapheme_boundary(&self.user_input, self.cursor_bidx);
        }
        self.cursor_gidx = bidx_to_gidx(&self.user_input, self.cursor_bidx);
        self.filter();
        self.refresh();
    }

    pub(super) fn resize(&mut self, window_size: Size2D<u32, PixelSize>) {
        self.window_size = window_size;
        self.refresh();
    }

    fn filter(&mut self) {
        self.filtered.clear();
        for choice in &self.choices {
            if let Some(score) = fuzzy_search(choice, &self.user_input) {
                self.filtered.push((score, choice.to_owned()));
            }
        }
        self.filtered.sort_by(|a, b| {
            if a.0 == b.0 {
                a.1.len().cmp(&b.1.len())
            } else {
                a.0.cmp(&b.0)
            }
        })
    }

    fn refresh(&mut self) {
        let max_height = (self.max_height_percentage * self.window_size.height) / 100;
        self.height = self.input_line.metrics.height + self.edge_padding * 2;
        self.lines.clear();
        let font_core = &mut *self.font_core.borrow_mut();

        self.input_line = if self.user_input.len() == 0 {
            FuzzyPopupLine::from_textstr(
                TextSpan::new(
                    " ",
                    self.text_size,
                    TextStyle::default(),
                    self.foreground_color,
                    TextPitch::Variable,
                    None,
                ),
                self.face,
                self.face,
                font_core,
                self.dpi,
            )
        } else {
            FuzzyPopupLine::from_textstr(
                TextSpan::new(
                    &self.user_input,
                    self.text_size,
                    TextStyle::default(),
                    self.foreground_color,
                    TextPitch::Variable,
                    None,
                ),
                self.face,
                self.face,
                font_core,
                self.dpi,
            )
        };
        for (_, line) in &self.filtered {
            let fmtline = FuzzyPopupLine::from_textstr(
                TextSpan::new(
                    line,
                    self.text_size,
                    TextStyle::default(),
                    self.foreground_color,
                    TextPitch::Variable,
                    None,
                ),
                self.face,
                self.face,
                font_core,
                self.dpi,
            );
            if self.height + self.bottom_off + fmtline.metrics.height > max_height {
                break;
            }
            self.height += fmtline.metrics.height;
            self.lines.push(fmtline);
        }
    }
}

fn is_grapheme_boundary(s: &str, idx: usize) -> bool {
    let mut gc = GraphemeCursor::new(idx, s.len(), true);
    gc.is_boundary(s, 0).unwrap()
}

fn next_grapheme_boundary(s: &str, idx: usize) -> usize {
    let mut gc = GraphemeCursor::new(idx, s.len(), true);
    gc.next_boundary(s, 0).unwrap().unwrap_or(s.len())
}

fn bidx_to_gidx(s: &str, bidx: usize) -> usize {
    let mut gidx = 0;
    for (i, _) in s.grapheme_indices(true) {
        if i >= bidx {
            return gidx;
        }
        gidx += 1;
    }
    gidx
}

fn fuzzy_search(haystack: &str, needle: &str) -> Option<usize> {
    let mut score = 0;
    let mut hci = haystack.char_indices();
    for nc in needle.chars() {
        let mut found = false;
        while let Some((i, hc)) = hci.next() {
            if hc == nc {
                score += i;
                found = true;
                break;
            }
        }
        if !found {
            return None;
        }
    }
    Some(score)
}
