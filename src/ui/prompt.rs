// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::rc::Rc;

use euclid::{point2, size2, Rect, SideOffsets2D, Size2D};
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

use crate::config::Cfg;
use crate::font::FontCore;
use crate::types::{PixelSize, TextPitch, TextStyle, DPI};

use super::context::ActiveRenderCtx;
use super::text::{ShapedTextLine, TextCursorStyle, TextLine, TextSpan};

pub(super) struct Prompt {
    is_active: bool,
    window_rect: Rect<u32, PixelSize>,
    height: u32,
    font_core: Rc<RefCell<FontCore>>,
    config: Rc<RefCell<Cfg>>,
    buffer: String,
    shaped: ShapedTextLine,
    cursor_bidx: usize,
    cursor_gidx: usize,
    dpi: Size2D<u32, DPI>,
}

impl Prompt {
    pub(super) fn new(
        window_rect: Rect<u32, PixelSize>,
        font_core: Rc<RefCell<FontCore>>,
        config: Rc<RefCell<Cfg>>,
        dpi: Size2D<u32, DPI>,
    ) -> Prompt {
        let mut ret = Prompt {
            window_rect: window_rect,
            height: 0,
            font_core: font_core,
            config: config,
            is_active: false,
            buffer: String::new(),
            shaped: ShapedTextLine::default(),
            cursor_bidx: 0,
            cursor_gidx: 0,
            dpi: dpi,
        };
        ret.refresh();
        ret
    }

    pub(super) fn draw(&mut self, actx: &mut ActiveRenderCtx) {
        let cfg = &*self.config.borrow();
        let cfguipr = &cfg.ui.fuzzy;
        let cfgprtheme = &cfg.ui.theme().fuzzy;

        let width = (self.window_rect.size.width * cfguipr.width_percentage) / 100;
        let lpad = (self.window_rect.size.width - width) / 2;
        let origin = point2(
            self.window_rect.origin.x + lpad,
            self.window_rect.origin.y + self.window_rect.size.height
                - self.height
                - cfguipr.bottom_offset,
        );
        let size = size2(width, self.height);
        let side_offsets = SideOffsets2D::new(
            cfgprtheme.edge_padding,
            cfgprtheme.edge_padding,
            cfgprtheme.edge_padding,
            cfgprtheme.edge_padding,
        );
        let rect = Rect::new(origin, size);
        let inner_rect = rect.inner_rect(side_offsets);

        {
            let size = size2(rect.size.width + 3, rect.size.height + 3);
            let shadow_rect = Rect::new(rect.origin, size);
            actx.draw_shadow(shadow_rect.cast());
            let _ctx = actx.get_widget_context(rect.cast(), cfgprtheme.background_color);
        }

        let font_core = &mut *self.font_core.borrow_mut();
        let mut ctx = actx.get_widget_context(inner_rect.cast(), cfgprtheme.background_color);
        let mut pos = point2(0, inner_rect.size.height as i32);
        pos.y += self.shaped.metrics.descender;

        // Draw buffer
        self.shaped.draw(
            &mut ctx,
            self.shaped.metrics.ascender,
            self.shaped.metrics.height as i32,
            pos,
            font_core,
            Some((
                self.cursor_gidx,
                TextCursorStyle::Beam,
                cfgprtheme.cursor_color,
                cfgprtheme.foreground_color,
            )),
        );
    }

    pub(super) fn set_window_rect(&mut self, window_rect: Rect<u32, PixelSize>) {
        self.window_rect = window_rect;
    }

    pub(super) fn is_active(&self) -> bool {
        self.is_active
    }

    pub(super) fn set_active(&mut self, val: bool) {
        self.is_active = val;
    }

    pub(super) fn set_string(&mut self, s: &str) {
        self.buffer.replace_range(.., s);
        self.cursor_bidx = s.len();
        self.cursor_gidx = bidx_to_gidx(&self.buffer, self.cursor_bidx);
        self.refresh();
    }

    pub(super) fn get_string(&self) -> &str {
        &self.buffer
    }

    pub(super) fn insert(&mut self, c: char) {
        self.buffer.push(c);
        self.cursor_bidx = next_grapheme_boundary(&self.buffer, self.cursor_bidx);
        self.cursor_gidx = bidx_to_gidx(&self.buffer, self.cursor_bidx);
        self.refresh();
    }

    pub(super) fn delete_left(&mut self) {
        if self.cursor_bidx == 0 {
            return;
        }
        let cur = self.cursor_bidx;
        self.cursor_bidx = 0;
        for (i, _) in self.buffer.char_indices() {
            if i >= cur {
                break;
            }
            self.cursor_bidx = i;
        }
        self.buffer.remove(self.cursor_bidx);
        if !is_grapheme_boundary(&self.buffer, self.cursor_bidx) {
            self.cursor_bidx = next_grapheme_boundary(&self.buffer, self.cursor_bidx);
        }
        self.cursor_gidx = bidx_to_gidx(&self.buffer, self.cursor_bidx);
        self.refresh();
    }

    fn refresh(&mut self) {
        let cfg = &*self.config.borrow();
        let cfguipr = &cfg.ui.prompt;
        let cfgprtheme = &cfg.ui.theme().prompt;
        let font_core = &mut *self.font_core.borrow_mut();

        self.shaped = if self.buffer.len() == 0 {
            ShapedTextLine::from_textstr(
                TextSpan::new(
                    " ",
                    cfguipr.text_size,
                    TextStyle::default(),
                    cfgprtheme.foreground_color,
                    TextPitch::Variable,
                    None,
                ),
                cfguipr.fixed_face,
                cfguipr.variable_face,
                font_core,
                self.dpi,
            )
        } else {
            ShapedTextLine::from_textstr(
                TextSpan::new(
                    &self.buffer,
                    cfguipr.text_size,
                    TextStyle::default(),
                    cfgprtheme.foreground_color,
                    TextPitch::Variable,
                    None,
                ),
                cfguipr.fixed_face,
                cfguipr.variable_face,
                font_core,
                self.dpi,
            )
        };
        self.height = self.shaped.metrics.height + cfgprtheme.edge_padding * 2;
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
