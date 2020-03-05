// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cmp::min;

use euclid::{point2, size2, Point2D, Rect, Size2D};
use unicode_segmentation::UnicodeSegmentation;

use crate::types::{Color, PixelSize, TextPitch, TextSize, TextStyle, DPI};

use super::context::WidgetRenderCtx;
use super::font::{harfbuzz, FaceKey, FontCore, ScaledFaceMetrics};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextCursorStyle {
    Beam,
    Block,
    Underline,
}

#[derive(Clone, Debug)]
pub(crate) struct TextSpan<'a> {
    pub(crate) data: &'a str,
    pub(crate) size: TextSize,
    pub(crate) style: TextStyle,
    pub(crate) color: Color,
    pub(crate) pitch: TextPitch,
    pub(crate) underline_color: Option<Color>,
}

impl<'a> TextSpan<'a> {
    pub(crate) fn new(
        data: &str,
        size: TextSize,
        style: TextStyle,
        color: Color,
        pitch: TextPitch,
        underline_color: Option<Color>,
    ) -> TextSpan {
        TextSpan {
            data: data,
            size: size,
            style: style,
            color: color,
            pitch: pitch,
            underline_color: underline_color,
        }
    }

    pub(super) fn base_face_metrics(
        &self,
        fixed_face: FaceKey,
        variable_face: FaceKey,
        font_core: &mut FontCore,
        dpi: Size2D<u32, DPI>,
    ) -> ScaledFaceMetrics {
        let base_face = match self.pitch {
            TextPitch::Fixed => fixed_face,
            TextPitch::Variable => variable_face,
        };
        let (_, face) = font_core.get(base_face, self.style).unwrap();
        face.raster.get_metrics(self.size, dpi)
    }

    pub(super) fn shaped_spans<'b>(
        &'a self,
        fixed_face: FaceKey,
        variable_face: FaceKey,
        font_core: &'b mut FontCore,
        dpi: Size2D<u32, DPI>,
    ) -> ShapedTextSpanIter<'a, 'b> {
        ShapedTextSpanIter {
            span: self,
            bidx: 0,
            font_core: font_core,
            base_face: match self.pitch {
                TextPitch::Fixed => fixed_face,
                TextPitch::Variable => variable_face,
            },
            dpi: dpi,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TextLine<'a>(pub(crate) Vec<TextSpan<'a>>);

pub(super) struct ShapedTextSpanIter<'a, 'b> {
    span: &'a TextSpan<'a>,
    bidx: usize,
    font_core: &'b mut FontCore,
    base_face: FaceKey,
    dpi: Size2D<u32, DPI>,
}

impl<'a, 'b> Iterator for ShapedTextSpanIter<'a, 'b> {
    type Item = ShapedTextSpan;

    fn next(&mut self) -> Option<ShapedTextSpan> {
        if self.bidx >= self.span.data.len() {
            return None;
        }

        let data = &self.span.data[self.bidx..];
        let mut cidxs = data.char_indices().peekable();
        let (face_key, c) = {
            let (_, c) = cidxs.next().unwrap();
            (
                self.font_core
                    .find_for_char(self.base_face, c)
                    .unwrap_or(self.base_face),
                c,
            )
        };

        let (buf, face) = self.font_core.get(face_key, self.span.style).unwrap();
        buf.clear_contents();
        buf.add(c, 0);
        let mut cluster = 1;
        let face_metrics = face.raster.get_metrics(self.span.size, self.dpi);

        while let Some((i, c)) = cidxs.peek() {
            if face.raster.has_glyph_for_char(*c) {
                buf.add(*c, cluster);
                cluster += 1;
                cidxs.next();
                continue;
            }

            face.shaper.set_scale(self.span.size, self.dpi);
            buf.guess_segment_properties();

            let mut last_cursor_position = 0;
            let cursor_positions = data[..*i]
                .graphemes(true)
                .map(|g| {
                    let num_chars = g.chars().count();
                    let ret = last_cursor_position;
                    last_cursor_position += num_chars;
                    ret
                })
                .collect();

            let ret = Some(ShapedTextSpan {
                face: face_key,
                color: self.span.color,
                size: self.span.size,
                style: self.span.style,
                cursor_positions: cursor_positions,
                glyph_infos: harfbuzz::shape(&face.shaper, buf).collect(),
                metrics: face_metrics,
                underline_color: self.span.underline_color,
            });

            self.bidx += *i;
            return ret;
        }

        face.shaper.set_scale(self.span.size, self.dpi);
        buf.guess_segment_properties();

        let mut last_cursor_position = 0;
        let cursor_positions = data
            .graphemes(true)
            .map(|g| {
                let num_chars = g.chars().count();
                let ret = last_cursor_position;
                last_cursor_position += num_chars;
                ret
            })
            .collect();

        let mut glyph_infos = Vec::new();
        for gi in harfbuzz::shape(&face.shaper, buf) {
            glyph_infos.push(gi);
        }
        let ret = Some(ShapedTextSpan {
            face: face_key,
            color: self.span.color,
            size: self.span.size,
            style: self.span.style,
            cursor_positions: cursor_positions,
            metrics: face_metrics,
            glyph_infos: glyph_infos,
            underline_color: self.span.underline_color,
        });

        self.bidx = self.span.data.len();
        ret
    }
}

#[derive(Debug)]
pub(super) struct ShapedTextSpan {
    pub(super) face: FaceKey,
    pub(super) color: Color,
    pub(super) size: TextSize,
    pub(super) style: TextStyle,
    pub(super) cursor_positions: Vec<usize>,
    pub(super) glyph_infos: Vec<harfbuzz::GlyphInfo>,
    pub(super) metrics: ScaledFaceMetrics,
    pub(super) underline_color: Option<Color>,
}

impl ShapedTextSpan {
    pub(super) fn clusters(&self) -> ShapedClusterIter {
        ShapedClusterIter {
            cursor_positions: &self.cursor_positions,
            glyph_infos: &self.glyph_infos,
            cpi: 0,
            gii: 0,
        }
    }
}

#[derive(Debug)]
pub(super) struct ShapedClusterIter<'a> {
    cursor_positions: &'a [usize],
    cpi: usize,
    glyph_infos: &'a [harfbuzz::GlyphInfo],
    gii: usize,
}

impl<'a> Iterator for ShapedClusterIter<'a> {
    type Item = ShapedCluster<'a>;

    fn next(&mut self) -> Option<ShapedCluster<'a>> {
        if self.cpi == self.cursor_positions.len() || self.gii == self.glyph_infos.len() {
            return None;
        }
        let mut i = self.gii + 1;
        while i < self.glyph_infos.len()
            && self.glyph_infos[i].cluster == self.glyph_infos[self.gii].cluster
        {
            i += 1;
        }
        if i == self.glyph_infos.len() {
            let ret = Some(ShapedCluster {
                num_graphemes: self.cursor_positions.len() - self.cpi,
                glyph_infos: &self.glyph_infos[self.gii..],
            });
            self.cpi = self.cursor_positions.len();
            self.gii = self.glyph_infos.len();
            ret
        } else {
            let mut count = 0;
            while self.cpi < self.cursor_positions.len()
                && self.cursor_positions[self.cpi] != self.glyph_infos[i].cluster as usize
            {
                self.cpi += 1;
                count += 1;
            }
            let ret = Some(ShapedCluster {
                num_graphemes: count,
                glyph_infos: &self.glyph_infos[self.gii..i],
            });
            self.gii = i;
            ret
        }
    }
}

#[derive(Debug)]
pub(super) struct ShapedCluster<'a> {
    pub(super) num_graphemes: usize,
    pub(super) glyph_infos: &'a [harfbuzz::GlyphInfo],
}

#[derive(Debug, Default)]
pub(super) struct ShapedTextLineMetrics {
    pub(super) ascender: i32,
    pub(super) descender: i32,
    pub(super) height: u32,
    pub(super) width: u32,
}

#[derive(Debug, Default)]
pub(super) struct ShapedTextLine {
    pub(super) metrics: ShapedTextLineMetrics,
    pub(super) spans: Vec<ShapedTextSpan>,
}

impl ShapedTextLine {
    pub(super) fn from_textline(
        line: TextLine,
        fixed_face: FaceKey,
        variable_face: FaceKey,
        font_core: &mut FontCore,
        dpi: Size2D<u32, DPI>,
    ) -> ShapedTextLine {
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
        let metrics = ShapedTextLineMetrics {
            ascender: ascender,
            descender: descender,
            height: (ascender - descender) as u32,
            width: if width < 0 { 0 } else { width as u32 },
        };
        ShapedTextLine {
            spans: spans,
            metrics: metrics,
        }
    }

    pub(super) fn from_textstr(
        span: TextSpan,
        fixed_face: FaceKey,
        variable_face: FaceKey,
        font_core: &mut FontCore,
        dpi: Size2D<u32, DPI>,
    ) -> ShapedTextLine {
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
        let metrics = ShapedTextLineMetrics {
            ascender: ascender,
            descender: descender,
            height: (ascender - descender) as u32,
            width: if width < 0 { 0 } else { width as u32 },
        };
        ShapedTextLine {
            spans: spans,
            metrics: metrics,
        }
    }

    pub(super) fn draw(
        &self,
        ctx: &mut WidgetRenderCtx,
        ascender: i32,
        height: i32,
        mut baseline: Point2D<i32, PixelSize>,
        font_core: &mut FontCore,
        cursor: Option<(usize, TextCursorStyle, Color)>,
    ) -> Point2D<i32, PixelSize> {
        let mut grapheme = 0;
        let mut block_cursor_width = 10;
        let mut underline_y = baseline.y;
        let mut underline_thickness = 1;

        for span in self.spans.iter() {
            underline_y = baseline.y - span.metrics.underline_pos;
            underline_thickness = span.metrics.underline_thickness;
            block_cursor_width = min(block_cursor_width, span.metrics.advance_width);

            let (_, face) = font_core.get(span.face, span.style).unwrap();
            for cluster in span.clusters() {
                if let Some((gidx, style, cursor_color)) = cursor {
                    if gidx >= grapheme && gidx < grapheme + cluster.num_graphemes {
                        let num_glyphs = cluster.glyph_infos.len();
                        if num_glyphs % cluster.num_graphemes != 0 {
                            let startx = baseline.x;
                            for gi in cluster.glyph_infos {
                                ctx.glyph(
                                    baseline + gi.offset,
                                    span.face,
                                    gi.gid,
                                    span.size,
                                    span.color,
                                    span.style,
                                    &mut face.raster,
                                );
                                baseline.x += gi.advance.width;
                            }
                            let width = baseline.x - startx;
                            let grapheme_width = width / cluster.num_graphemes as i32;
                            let cursor_x = startx + ((gidx - grapheme) as i32) * grapheme_width;
                            let (cursor_y, cursor_size) = match style {
                                TextCursorStyle::Beam => (baseline.y - ascender, size2(2, height)),
                                TextCursorStyle::Block => {
                                    (baseline.y - ascender, size2(grapheme_width, height))
                                }
                                TextCursorStyle::Underline => {
                                    (underline_y, size2(grapheme_width, underline_thickness))
                                }
                            };
                            ctx.color_quad(
                                Rect::new(point2(cursor_x, cursor_y), cursor_size),
                                cursor_color,
                            );
                            grapheme += cluster.num_graphemes;
                        } else {
                            let glyphs_per_grapheme = num_glyphs / cluster.num_graphemes;
                            for i in (0..num_glyphs).step_by(glyphs_per_grapheme) {
                                let mut draw_cursor = false;
                                if gidx == grapheme {
                                    draw_cursor = true;
                                }
                                let cursor_x = baseline.x;
                                for gi in &cluster.glyph_infos[i..(i + glyphs_per_grapheme)] {
                                    ctx.glyph(
                                        baseline + gi.offset,
                                        span.face,
                                        gi.gid,
                                        span.size,
                                        span.color,
                                        span.style,
                                        &mut face.raster,
                                    );
                                    baseline.x += gi.advance.width;
                                }
                                let width = baseline.x - cursor_x;
                                if draw_cursor {
                                    let (cursor_y, cursor_size) = match style {
                                        TextCursorStyle::Beam => {
                                            (baseline.y - ascender, size2(2, height))
                                        }
                                        TextCursorStyle::Block => {
                                            (baseline.y - ascender, size2(width, height))
                                        }
                                        TextCursorStyle::Underline => {
                                            (underline_y, size2(width, underline_thickness))
                                        }
                                    };
                                    ctx.color_quad(
                                        Rect::new(point2(cursor_x, cursor_y), cursor_size),
                                        cursor_color,
                                    );
                                }
                                grapheme += 1;
                            }
                        }
                        continue;
                    }
                }
                for gi in cluster.glyph_infos {
                    ctx.glyph(
                        baseline + gi.offset,
                        span.face,
                        gi.gid,
                        span.size,
                        span.color,
                        span.style,
                        &mut face.raster,
                    );
                    baseline.x += gi.advance.width;
                }
                grapheme += cluster.num_graphemes;
            }
        }
        if let Some((gidx, style, cursor_color)) = cursor {
            if gidx == grapheme {
                let (cursor_y, cursor_size) = match style {
                    TextCursorStyle::Beam => (baseline.y - ascender, size2(2, height)),
                    TextCursorStyle::Block => {
                        (baseline.y - ascender, size2(block_cursor_width, height))
                    }
                    TextCursorStyle::Underline => {
                        (underline_y, size2(block_cursor_width, underline_thickness))
                    }
                };
                ctx.color_quad(
                    Rect::new(point2(baseline.x, cursor_y), cursor_size),
                    cursor_color,
                );
            }
        }
        baseline
    }
}
