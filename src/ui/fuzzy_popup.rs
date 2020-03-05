// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::cmp::{max, min};
use std::rc::Rc;

use euclid::{point2, size2, Rect, SideOffsets2D, Size2D};
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

use crate::types::{Color, PixelSize, TextPitch, TextSize, TextStyle, DPI};

use super::context::ActiveRenderCtx;
use super::font::{FaceKey, FontCore};
use super::text::{ShapedTextLine, TextCursorStyle, TextSpan};

pub(super) struct FuzzyPopup {
    window_size: Size2D<u32, PixelSize>,
    max_height_percentage: u32,
    width_percentage: u32,
    height: u32,
    edge_padding: u32,
    bottom_off: u32,
    line_spacing: u32,
    background_color: Color,
    foreground_color: Color,
    label_color: Color,
    selected_color: Color,
    cursor_color: Color,
    text_size: TextSize,
    face: FaceKey,
    input_line: ShapedTextLine,
    input_label: ShapedTextLine,
    lines: Vec<ShapedTextLine>,
    dpi: Size2D<u32, DPI>,
    font_core: Rc<RefCell<FontCore>>,
    input_label_str: String,
    user_input: String,
    choices: Vec<String>,
    filtered: Vec<(usize, String)>,
    is_active: bool,
    default_on_empty: bool,
    cursor_bidx: usize,
    cursor_gidx: usize,
}

impl FuzzyPopup {
    pub(super) fn new(
        window_size: Size2D<u32, PixelSize>,
        max_height_percentage: u32,
        width_percentage: u32,
        edge_padding: u32,
        bottom_off: u32,
        line_spacing: u32,
        background_color: Color,
        foreground_color: Color,
        label_color: Color,
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
            bottom_off: bottom_off,
            line_spacing: line_spacing,
            height: 0,
            background_color: background_color,
            foreground_color: foreground_color,
            label_color: label_color,
            selected_color: selected_color,
            cursor_color: cursor_color,
            text_size: text_size,
            face: face,
            input_line: ShapedTextLine::default(),
            input_label: ShapedTextLine::default(),
            lines: Vec::new(),
            dpi: dpi,
            font_core: font_core,
            input_label_str: String::new(),
            user_input: String::new(),
            choices: Vec::new(),
            filtered: Vec::new(),
            is_active: false,
            default_on_empty: false,
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
            let size = size2(rect.size.width + 5, rect.size.height + 5);
            let shadow_rect = Rect::new(rect.origin, size);
            actx.draw_shadow(shadow_rect.cast());
            let _ctx = actx.get_widget_context(rect.cast(), self.background_color);
        }

        let font_core = &mut *self.font_core.borrow_mut();
        let mut ctx = actx.get_widget_context(inner_rect.cast(), self.background_color);
        let mut pos = point2(0, inner_rect.size.height as i32);
        pos.y += min(
            self.input_line.metrics.descender,
            self.input_label.metrics.descender,
        ) as i32;

        // Draw input label
        let mut pos_here = self.input_label.draw(
            &mut ctx,
            self.input_label.metrics.ascender,
            self.input_label.metrics.height as i32,
            pos,
            font_core,
            None,
        );
        pos_here.x += 5;
        let text_padding = pos_here.x;

        // Draw input line
        self.input_line.draw(
            &mut ctx,
            self.input_line.metrics.ascender,
            self.input_line.metrics.height as i32,
            pos_here,
            font_core,
            Some((self.cursor_gidx, TextCursorStyle::Beam, self.cursor_color)),
        );
        pos.y -= max(
            self.input_line.metrics.ascender,
            self.input_label.metrics.ascender,
        ) as i32;

        // Draw selection lines
        if self.lines.len() > 0 {
            pos.y -= (self.lines[0].metrics.height + 2 * self.line_spacing) as i32;
            let rect = Rect::new(pos, size2(width, self.lines[0].metrics.height).cast());
            ctx.color_quad(rect, Color::new(0, 0, 0, 8));

            for i in 0..self.lines.len() {
                let line = &self.lines[i];
                if i > 0 {
                    pos.y -= (line.metrics.height + 2 * self.line_spacing) as i32;
                }
                let mut pos_here = pos;
                pos_here.x += text_padding;
                pos_here.y += line.metrics.ascender;
                line.draw(
                    &mut ctx,
                    line.metrics.ascender,
                    line.metrics.height as i32,
                    pos_here,
                    font_core,
                    None,
                );
            }
        }
    }

    pub(super) fn push_string_choices(&mut self, choices: &[String]) {
        self.choices.extend_from_slice(choices);
        self.filter();
        self.refresh();
    }

    pub(super) fn push_str_choices(&mut self, choices: &[&str]) {
        for s in choices {
            self.choices.push(s.to_string());
        }
        self.filter();
        self.refresh();
    }

    pub(super) fn is_active(&self) -> bool {
        self.is_active
    }

    pub(super) fn set_input_label(&mut self, label: &str) {
        self.input_label_str = label.to_owned();
        self.refresh();
    }

    pub(super) fn set_default_on_empty(&mut self, val: bool) {
        self.default_on_empty = val;
    }

    pub(super) fn set_active(&mut self, val: bool) {
        self.is_active = val;
        self.choices.clear();
        self.user_input.clear();
        self.filtered.clear();
        self.cursor_bidx = 0;
        self.cursor_gidx = 0;
    }

    pub(super) fn get_selection(&self) -> Option<String> {
        if self.filtered.len() > 0 {
            if self.default_on_empty || self.user_input.len() > 0 {
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
                //a.1.len().cmp(&b.1.len())
                a.1.cmp(&b.1)
            } else {
                a.0.cmp(&b.0)
            }
        })
    }

    fn refresh(&mut self) {
        let max_height = (self.max_height_percentage * self.window_size.height) / 100;
        self.lines.clear();
        let font_core = &mut *self.font_core.borrow_mut();

        self.input_line = if self.user_input.len() == 0 {
            ShapedTextLine::from_textstr(
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
            ShapedTextLine::from_textstr(
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

        self.input_label = if self.input_label_str.len() == 0 {
            ShapedTextLine::from_textstr(
                TextSpan::new(
                    " ",
                    self.text_size,
                    TextStyle::default(),
                    self.label_color,
                    TextPitch::Variable,
                    None,
                ),
                self.face,
                self.face,
                font_core,
                self.dpi,
            )
        } else {
            ShapedTextLine::from_textstr(
                TextSpan::new(
                    &self.input_label_str,
                    self.text_size,
                    TextStyle::default(),
                    self.label_color,
                    TextPitch::Variable,
                    None,
                ),
                self.face,
                self.face,
                font_core,
                self.dpi,
            )
        };

        self.height = max(
            self.input_line.metrics.height,
            self.input_label.metrics.height,
        ) + self.edge_padding * 2
            + self.line_spacing;

        for (_, line) in &self.filtered {
            let fmtline = ShapedTextLine::from_textstr(
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
            if self.height + self.bottom_off + self.line_spacing * 2 + fmtline.metrics.height
                > max_height
            {
                break;
            }
            self.height += fmtline.metrics.height + self.line_spacing * 2;
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
    for split in needle.split_whitespace() {
        for nc in split.chars() {
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
    }
    Some(score)
}
