// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::cmp::{max, min};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError};

use euclid::{point2, size2, Rect, SideOffsets2D, Size2D};
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

use crate::config::Cfg;
use crate::font::FontCore;
use crate::types::{PixelSize, TextPitch, TextStyle, DPI};

use super::context::ActiveRenderCtx;
use super::text::{ShapedTextLine, TextCursorStyle, TextLine, TextSpan};

pub(super) struct FuzzyPopup {
    is_active: bool,
    interacted: bool,
    pub(super) to_refresh: bool,
    window_rect: Rect<u32, PixelSize>,
    height: u32,
    input_line: ShapedTextLine,
    input_label: ShapedTextLine,
    lines: Vec<ShapedTextLine>,
    dpi: Size2D<u32, DPI>,
    input_label_str: String,
    user_input: String,
    choices: Vec<String>,
    filtered: Vec<(usize, String, Vec<(usize, usize)>)>,
    select_idx: usize,
    default_on_empty: bool,
    cursor_bidx: usize,
    cursor_gidx: usize,
    font_core: Rc<RefCell<FontCore>>,
    config: Rc<RefCell<Cfg>>,
    async_source: Option<Receiver<String>>,
}

impl FuzzyPopup {
    pub(super) fn new(
        window_rect: Rect<u32, PixelSize>,
        font_core: Rc<RefCell<FontCore>>,
        config: Rc<RefCell<Cfg>>,
        dpi: Size2D<u32, DPI>,
    ) -> FuzzyPopup {
        let mut ret = FuzzyPopup {
            window_rect: window_rect,
            height: 0,
            input_line: ShapedTextLine::default(),
            input_label: ShapedTextLine::default(),
            lines: Vec::new(),
            dpi: dpi,
            font_core: font_core,
            config: config,
            input_label_str: String::new(),
            user_input: String::new(),
            choices: Vec::new(),
            filtered: Vec::new(),
            select_idx: 0,
            is_active: false,
            interacted: false,
            to_refresh: false,
            default_on_empty: false,
            cursor_bidx: 0,
            cursor_gidx: 0,
            async_source: None,
        };
        ret.refresh();
        ret
    }

    pub(super) fn draw(&mut self, actx: &mut ActiveRenderCtx) {
        self.to_refresh = false;
        let cfg = &*self.config.borrow();
        let cfguifz = &cfg.ui.fuzzy;
        let cfgfztheme = &cfg.ui.theme().fuzzy;

        let width = (self.window_rect.size.width * cfguifz.width_percentage) / 100;
        let lpad = (self.window_rect.size.width - width) / 2;
        let origin = point2(
            self.window_rect.origin.x + lpad,
            self.window_rect.origin.y + self.window_rect.size.height
                - self.height
                - cfguifz.bottom_offset,
        );
        let size = size2(width, self.height);
        let side_offsets = SideOffsets2D::new(
            cfgfztheme.edge_padding,
            cfgfztheme.edge_padding,
            cfgfztheme.edge_padding,
            cfgfztheme.edge_padding,
        );
        let rect = Rect::new(origin, size);
        let inner_rect = rect.inner_rect(side_offsets);

        {
            let size = size2(rect.size.width + 3, rect.size.height + 3);
            let shadow_rect = Rect::new(rect.origin, size);
            actx.draw_shadow(shadow_rect.cast());
            let _ctx = actx.get_widget_context(rect.cast(), cfgfztheme.background_color);
        }

        let font_core = &mut *self.font_core.borrow_mut();
        let mut ctx = actx.get_widget_context(inner_rect.cast(), cfgfztheme.background_color);
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
            100,
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
            Some((
                self.cursor_gidx,
                TextCursorStyle::Beam,
                cfgfztheme.cursor_color,
                cfgfztheme.foreground_color,
            )),
            100,
        );
        pos.y -= max(
            self.input_line.metrics.ascender,
            self.input_label.metrics.ascender,
        ) as i32;

        // Draw selection lines
        if self.lines.len() > 0 {
            for i in 0..self.lines.len() {
                let line = &self.lines[i];
                pos.y -= (line.metrics.height + 2 * cfguifz.line_spacing) as i32;

                if i == self.select_idx {
                    let rect = Rect::new(pos, size2(width, self.lines[i].metrics.height).cast());
                    ctx.color_quad(rect, cfgfztheme.select_background_color);
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
                    100,
                );
            }
        }
    }

    pub(super) fn set_async_source(&mut self, source: Receiver<String>) {
        self.async_source = Some(source);
    }

    pub(super) fn update_from_async(&mut self) {
        let mut found = false;
        let start = self.choices.len();
        if let Some(source) = &self.async_source {
            loop {
                match source.try_recv() {
                    Ok(s) => {
                        self.choices.push(s);
                        found = true;
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.async_source = None;
                        break;
                    }
                    _ => break,
                }
            }
        }
        if found {
            for choice in &self.choices[start..] {
                if let Some((score, indices)) = fuzzy_search(choice, &self.user_input) {
                    self.filtered.push((score, choice.to_owned(), indices));
                }
            }
            self.filtered.sort_by(|a, b| {
                if a.0 == b.0 {
                    //a.1.len().cmp(&b.1.len())
                    a.1.cmp(&b.1)
                } else {
                    a.0.cmp(&b.0)
                }
            });
            self.refresh();
            self.to_refresh = true;
        }
    }

    pub(super) fn re_filter(&mut self) {
        self.filter();
        self.refresh();
        self.to_refresh = true;
    }

    pub(super) fn push_string_choices(&mut self, choices: &[String]) {
        self.choices.extend_from_slice(choices);
        self.to_refresh = true;
    }

    pub(super) fn push_str_choices(&mut self, choices: &[&str]) {
        for s in choices {
            self.choices.push(s.to_string());
        }
        self.to_refresh = true;
    }

    pub(super) fn is_active(&self) -> bool {
        self.is_active
    }

    pub(super) fn set_input_label(&mut self, label: &str) {
        self.input_label_str = label.to_owned();
        self.refresh();
        self.to_refresh = true;
    }

    pub(super) fn set_default_on_empty(&mut self, val: bool) {
        self.default_on_empty = val;
    }

    pub(super) fn set_active(&mut self, val: bool) {
        self.async_source = None;
        self.is_active = val;
        self.interacted = false;
        self.choices.clear();
        self.user_input.clear();
        self.filtered.clear();
        self.select_idx = 0;
        self.cursor_bidx = 0;
        self.cursor_gidx = 0;
        self.to_refresh = true;
    }

    pub(super) fn get_selection(&self) -> Option<String> {
        if self.filtered.len() > 0 {
            if self.default_on_empty || self.interacted {
                Some(self.filtered[self.select_idx].1.to_owned())
            } else {
                None
            }
        } else {
            None
        }
    }

    pub(super) fn insert(&mut self, c: char) {
        self.interacted = true;
        self.user_input.push(c);
        self.cursor_bidx = next_grapheme_boundary(&self.user_input, self.cursor_bidx);
        self.cursor_gidx = bidx_to_gidx(&self.user_input, self.cursor_bidx);
        self.to_refresh = true;
    }

    pub(super) fn delete_left(&mut self) {
        self.interacted = true;
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
        self.to_refresh = true;
    }

    pub(super) fn up_key(&mut self) {
        self.interacted = true;
        self.select_idx += 1;
        if self.select_idx >= self.filtered.len() {
            self.select_idx = self.filtered.len() - 1;
        }
        self.to_refresh = true;
    }

    pub(super) fn down_key(&mut self) {
        self.interacted = true;
        if self.select_idx > 0 {
            self.select_idx -= 1;
        }
        self.to_refresh = true;
    }

    pub(super) fn tab_key(&mut self) {
        self.interacted = true;
        if self.filtered.len() > 0 {
            self.user_input = self.filtered[self.select_idx].1.clone();
            self.cursor_bidx = self.user_input.len();
            self.cursor_gidx = bidx_to_gidx(&self.user_input, self.cursor_bidx);
        }
        self.to_refresh = true;
    }

    pub(super) fn set_window_rect(&mut self, window_rect: Rect<u32, PixelSize>) {
        self.window_rect = window_rect;
        self.refresh();
        self.to_refresh = true;
    }

    fn filter(&mut self) {
        self.filtered.clear();
        self.select_idx = 0;
        for choice in &self.choices {
            if let Some((score, indices)) = fuzzy_search(choice, &self.user_input) {
                self.filtered.push((score, choice.to_owned(), indices));
            }
        }
        self.filtered.sort_by(|a, b| {
            if a.0 == b.0 {
                //a.1.len().cmp(&b.1.len())
                a.1.cmp(&b.1)
            } else {
                a.0.cmp(&b.0)
            }
        });
    }

    fn refresh(&mut self) {
        let cfg = &*self.config.borrow();
        let cfguifz = &cfg.ui.fuzzy;
        let cfgfztheme = &cfg.ui.theme().fuzzy;

        let max_height = (cfguifz.max_height_percentage * self.window_rect.size.height) / 100;
        self.lines.clear();
        let font_core = &mut *self.font_core.borrow_mut();

        self.input_line = if self.user_input.len() == 0 {
            ShapedTextLine::from_textstr(
                TextSpan::new(
                    " ",
                    cfguifz.text_size,
                    TextStyle::default(),
                    cfgfztheme.foreground_color,
                    TextPitch::Variable,
                    None,
                ),
                cfguifz.fixed_face,
                cfguifz.variable_face,
                font_core,
                self.dpi,
            )
        } else {
            ShapedTextLine::from_textstr(
                TextSpan::new(
                    &self.user_input,
                    cfguifz.text_size,
                    TextStyle::default(),
                    cfgfztheme.foreground_color,
                    TextPitch::Variable,
                    None,
                ),
                cfguifz.fixed_face,
                cfguifz.variable_face,
                font_core,
                self.dpi,
            )
        };

        self.input_label = if self.input_label_str.len() == 0 {
            ShapedTextLine::from_textstr(
                TextSpan::new(
                    " ",
                    cfguifz.text_size,
                    TextStyle::default(),
                    cfgfztheme.label_color,
                    TextPitch::Variable,
                    None,
                ),
                cfguifz.fixed_face,
                cfguifz.variable_face,
                font_core,
                self.dpi,
            )
        } else {
            ShapedTextLine::from_textstr(
                TextSpan::new(
                    &self.input_label_str,
                    cfguifz.text_size,
                    TextStyle::default(),
                    cfgfztheme.label_color,
                    TextPitch::Variable,
                    None,
                ),
                cfguifz.fixed_face,
                cfguifz.variable_face,
                font_core,
                self.dpi,
            )
        };

        self.height = max(
            self.input_line.metrics.height,
            self.input_label.metrics.height,
        ) + cfgfztheme.edge_padding * 2
            + cfguifz.line_spacing;

        let mut i = 0;
        for (_, line, indices) in &self.filtered {
            let match_color = if i == self.select_idx {
                cfgfztheme.select_match_color
            } else {
                cfgfztheme.match_color
            };
            let color = if i == self.select_idx {
                cfgfztheme.select_color
            } else {
                cfgfztheme.foreground_color
            };

            let mut textline = TextLine::default();
            let mut j = 0;
            for (start, end) in indices {
                if j < *start {
                    textline.0.push(TextSpan::new(
                        &line[j..*start],
                        cfguifz.text_size,
                        TextStyle::default(),
                        color,
                        TextPitch::Variable,
                        None,
                    ));
                }
                textline.0.push(TextSpan::new(
                    &line[*start..*end],
                    cfguifz.text_size,
                    TextStyle::default(),
                    match_color,
                    TextPitch::Variable,
                    None,
                ));
                j = *end;
            }
            if j < line.len() {
                textline.0.push(TextSpan::new(
                    &line[j..],
                    cfguifz.text_size,
                    TextStyle::default(),
                    color,
                    TextPitch::Variable,
                    None,
                ));
            }

            let fmtline = ShapedTextLine::from_textline(
                textline,
                cfguifz.fixed_face,
                cfguifz.variable_face,
                font_core,
                self.dpi,
            );
            if self.height
                + cfguifz.bottom_offset
                + cfguifz.line_spacing * 2
                + fmtline.metrics.height
                > max_height
            {
                break;
            }
            self.height += fmtline.metrics.height + cfguifz.line_spacing * 2;
            self.lines.push(fmtline);
            i += 1;
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

fn fuzzy_search(haystack: &str, needle: &str) -> Option<(usize, Vec<(usize, usize)>)> {
    let mut score = 0;
    let mut indices = Vec::new();
    for split in needle.split_whitespace() {
        let mut hci = haystack.char_indices();
        let mut start = None;
        for nc in split.chars() {
            let mut found = false;
            while let Some((i, hc)) = hci.next() {
                if hc == nc {
                    if let Some(start) = start {
                        score += i - start;
                    } else {
                        score += i;
                        start = Some(i);
                    }
                    found = true;
                    break;
                } else if start.is_some() {
                    indices.push((start.unwrap(), i));
                    start = None;
                }
            }
            if !found {
                return None;
            }
        }
        if start.is_some() {
            if let Some((i, _)) = hci.next() {
                indices.push((start.unwrap(), i));
            } else {
                indices.push((start.unwrap(), haystack.len()));
            }
        }
    }
    indices.sort_by(|a, b| a.0.cmp(&b.0));
    Some((score, indices))
}
