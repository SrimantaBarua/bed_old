// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use euclid::Size2D;
use ropey::RopeSlice;
use unicode_segmentation::UnicodeSegmentation;

use super::font::{harfbuzz, FaceKey, FontCore, ScaledFaceMetrics};
use crate::textbuffer::RopeGraphemes;
use crate::types::{Color, TextPitch, TextSize, TextStyle, DPI};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextCursorStyle {
    Beam,
    Block,
    Underline,
}

#[derive(Clone, Debug)]
pub(crate) struct TextSpan<'a> {
    pub(crate) data: RopeSlice<'a>,
    pub(crate) size: TextSize,
    pub(crate) style: TextStyle,
    pub(crate) color: Color,
    pub(crate) pitch: TextPitch,
    pub(crate) underline_color: Option<Color>,
}

impl<'a> TextSpan<'a> {
    pub(crate) fn new(
        data: RopeSlice<'a>,
        size: TextSize,
        style: TextStyle,
        color: Color,
        pitch: TextPitch,
        underline_color: Option<Color>,
    ) -> TextSpan<'a> {
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
            cidx: 0,
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
    cidx: usize,
    font_core: &'b mut FontCore,
    base_face: FaceKey,
    dpi: Size2D<u32, DPI>,
}

impl<'a, 'b> Iterator for ShapedTextSpanIter<'a, 'b> {
    type Item = ShapedTextSpan;

    fn next(&mut self) -> Option<ShapedTextSpan> {
        let len_chars = self.span.data.len_chars();
        if self.cidx >= len_chars {
            return None;
        }

        let data = self.span.data.slice(self.cidx..len_chars);
        let mut chars = data.chars().peekable();
        let (face_key, c) = {
            let c = chars.next().unwrap();
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

        while let Some(c) = chars.peek() {
            if face.raster.has_glyph_for_char(*c) {
                buf.add(*c, cluster);
                cluster += 1;
                chars.next();
                continue;
            }

            face.shaper.set_scale(self.span.size, self.dpi);
            buf.guess_segment_properties();

            let mut last_cursor_position = 0;
            let cursor_positions = RopeGraphemes::new(&data.slice(0..(cluster as usize)))
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

            self.cidx += cluster as usize;
            return ret;
        }

        face.shaper.set_scale(self.span.size, self.dpi);
        buf.guess_segment_properties();

        let mut last_cursor_position = 0;
        let cursor_positions = RopeGraphemes::new(&data)
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

        self.cidx = self.span.data.len_chars();
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
