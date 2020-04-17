// (C) 2019 Srimanta Barua <srimanta.barua1@gmail.com>

use std::cell::RefCell;
use std::rc::Rc;

use euclid::{size2, Rect, Size2D};

use crate::config::Cfg;
use crate::font::FontCore;
use crate::textbuffer::Buffer;
use crate::types::{PixelSize, DPI};

use super::context::ActiveRenderCtx;
use super::textview::TextView;

pub(super) struct TextViewTree {
    config: Rc<RefCell<Cfg>>,
    rect: Rect<u32, PixelSize>,
    root: Node,
}

impl TextViewTree {
    pub(super) fn new(
        buffer: Rc<RefCell<Buffer>>,
        rect: Rect<u32, PixelSize>,
        font_core: Rc<RefCell<FontCore>>,
        config: Rc<RefCell<Cfg>>,
        dpi: Size2D<u32, DPI>,
        line_numbers: bool,
        relative_number: bool,
        view_id: usize,
    ) -> TextViewTree {
        let leaf = Node::new_leaf(
            buffer,
            rect,
            font_core,
            config.clone(),
            dpi,
            line_numbers,
            relative_number,
            view_id,
        );
        TextViewTree {
            root: leaf,
            config: config,
            rect: rect,
        }
    }

    // Kill the current active pane. Return true if that was the last pane, false
    // otherwise
    pub(super) fn kill_active(&mut self) -> bool {
        if !self.root.kill_active() {
            let cfg = &*self.config.borrow();
            let borderwidth = cfg.ui.theme().textview.border_width;
            self.root.compute_rects(borderwidth);
            return false;
        }
        true
    }

    pub(super) fn draw(&mut self, active_ctx: &mut ActiveRenderCtx) {
        {
            let cfg = &*self.config.borrow();
            let theme = &cfg.ui.theme().textview;
            let bgcol = theme.background_color;
            let border_color = cfg.ui.theme().textview.border_color;
            let rect = self.rect.cast();
            let mut ctx = active_ctx.get_widget_context(rect, bgcol);
            ctx.color_quad(rect, border_color);
        }
        self.root.draw(active_ctx, true)
    }

    pub(super) fn move_cursor_to_point(&mut self, point: (i32, i32)) {
        self.root.move_cursor_to_point(point);
    }

    pub(super) fn scroll_views(
        &mut self,
        cursor: Option<(i32, i32)>,
        force: (f64, f64),
        time: f64,
    ) -> bool {
        self.root.scroll(cursor, force, time)
    }

    pub(super) fn set_rect(&mut self, rect: Rect<u32, PixelSize>) {
        let cfg = &*self.config.borrow();
        let borderwidth = cfg.ui.theme().textview.border_width;
        self.rect = rect;
        self.root.set_rect(rect, borderwidth);
    }

    pub(super) fn active_mut(&mut self) -> &mut TextView {
        self.root.active_mut()
    }

    pub(super) fn split_h(&mut self, view_id: usize) {
        let cfg = &*self.config.borrow();
        let borderwidth = cfg.ui.theme().textview.border_width;
        self.root.split_h(view_id);
        self.root.compute_rects(borderwidth);
    }

    pub(super) fn split_v(&mut self, view_id: usize) {
        let cfg = &*self.config.borrow();
        let borderwidth = cfg.ui.theme().textview.border_width;
        self.root.split_v(view_id);
        self.root.compute_rects(borderwidth);
    }
}

enum Node {
    InnerH(Vec<Node>, Rect<u32, PixelSize>, Option<usize>),
    InnerV(Vec<Node>, Rect<u32, PixelSize>, Option<usize>),
    Leaf(TextView),
}

impl Node {
    fn new_leaf(
        buffer: Rc<RefCell<Buffer>>,
        rect: Rect<u32, PixelSize>,
        font_core: Rc<RefCell<FontCore>>,
        config: Rc<RefCell<Cfg>>,
        dpi: Size2D<u32, DPI>,
        line_numbers: bool,
        relative_number: bool,
        view_id: usize,
    ) -> Node {
        Node::Leaf(TextView::new(
            buffer,
            rect,
            font_core,
            config,
            dpi,
            line_numbers,
            relative_number,
            view_id,
        ))
    }

    fn kill_active(&mut self) -> bool {
        match self {
            Node::Leaf(_) => true,
            Node::InnerH(v, _, i) | Node::InnerV(v, _, i) => {
                let j = i.unwrap();
                if v[j].kill_active() {
                    v.remove(j);
                }
                if j > 0 {
                    *i = Some(j - 1);
                }
                v.len() == 0
            }
        }
    }

    fn split_h(&mut self, view_id: usize) {
        match self {
            Node::Leaf(t) => {
                let rect = t.get_rect();
                let other = t.split(view_id);
                *self = Node::InnerH(
                    vec![Node::Leaf(other), Node::Leaf(t.clone())],
                    rect,
                    Some(0),
                );
            }
            Node::InnerH(v, _, i) => {
                let i = i.unwrap();
                match &mut v[i] {
                    Node::Leaf(t) => {
                        let other = t.split(view_id);
                        v.insert(i, Node::Leaf(other));
                    }
                    _ => v[i].split_h(view_id),
                }
            }
            Node::InnerV(v, _, i) => v[i.unwrap()].split_h(view_id),
        }
    }

    fn split_v(&mut self, view_id: usize) {
        match self {
            Node::Leaf(t) => {
                let rect = t.get_rect();
                let other = t.split(view_id);
                *self = Node::InnerV(
                    vec![Node::Leaf(other), Node::Leaf(t.clone())],
                    rect,
                    Some(0),
                );
            }
            Node::InnerV(v, _, i) => {
                let i = i.unwrap();
                match &mut v[i] {
                    Node::Leaf(t) => {
                        let other = t.split(view_id);
                        v.insert(i, Node::Leaf(other));
                    }
                    _ => v[i].split_v(view_id),
                }
            }
            Node::InnerH(v, _, i) => v[i.unwrap()].split_v(view_id),
        }
    }

    fn move_cursor_to_point(&mut self, point: (i32, i32)) {
        match self {
            Node::Leaf(t) => t.move_cursor_to_point(point),
            Node::InnerH(v, _, i) => {
                for j in 0..v.len() {
                    let rbox = v[j].get_rect().to_box2d().cast().to_untyped();
                    if point.0 < rbox.max.x {
                        v[j].move_cursor_to_point((point.0 - rbox.min.x, point.1));
                        *i = Some(j);
                        break;
                    }
                }
            }
            Node::InnerV(v, _, i) => {
                for j in 0..v.len() {
                    let rbox = v[j].get_rect().to_box2d().cast().to_untyped();
                    if point.1 < rbox.max.y {
                        v[j].move_cursor_to_point((point.0, point.1 - rbox.min.y));
                        *i = Some(j);
                        break;
                    }
                }
            }
        }
    }

    fn scroll(&mut self, mut cursor: Option<(i32, i32)>, force: (f64, f64), time: f64) -> bool {
        match self {
            Node::Leaf(t) => {
                if cursor.is_some() {
                    t.scroll(force, time)
                } else {
                    t.scroll((0.0, 0.0), time)
                }
            }
            Node::InnerH(v, _, _) => {
                let mut ret = false;
                for j in 0..v.len() {
                    let rbox = v[j].get_rect().to_box2d().cast().to_untyped();
                    if let Some(c) = cursor {
                        if c.0 < rbox.max.x {
                            ret |= v[j].scroll(Some((c.0 - rbox.min.x, c.1)), force, time);
                            cursor = None;
                        }
                    } else {
                        ret |= v[j].scroll(None, (0.0, 0.0), time);
                    };
                }
                ret
            }
            Node::InnerV(v, _, _) => {
                let mut ret = false;
                for j in 0..v.len() {
                    let rbox = v[j].get_rect().to_box2d().cast().to_untyped();
                    if let Some(c) = cursor {
                        if c.1 < rbox.max.y {
                            ret |= v[j].scroll(Some((c.0, c.1 - rbox.min.y)), force, time);
                            cursor = None;
                        }
                    } else {
                        ret |= v[j].scroll(None, (0.0, 0.0), time);
                    };
                }
                ret
            }
        }
    }

    fn set_rect(&mut self, rect: Rect<u32, PixelSize>, border_width: u32) {
        match self {
            Node::Leaf(t) => t.set_rect(rect),
            Node::InnerH(_, r, _) | Node::InnerV(_, r, _) => *r = rect,
        }
        self.compute_rects(border_width);
    }

    fn compute_rects(&mut self, border_width: u32) {
        match self {
            Node::Leaf(_) => {}
            Node::InnerH(v, r, _) => {
                let mut pos = r.origin;
                let height = r.size.height;
                let total_width = r.size.width - (v.len() as u32 - 1) * border_width;
                let width = total_width / v.len() as u32;
                let j = total_width as usize % v.len();
                for i in 0..j {
                    v[i].set_rect(Rect::new(pos, size2(width + 1, height)), border_width);
                    pos.x += width + 1 + border_width;
                }
                for i in j..v.len() {
                    v[i].set_rect(Rect::new(pos, size2(width, height)), border_width);
                    pos.x += width + border_width;
                }
            }
            Node::InnerV(v, r, _) => {
                let mut pos = r.origin;
                let width = r.size.width;
                let total_height = r.size.height - (v.len() as u32 - 1) * border_width;
                let height = total_height / v.len() as u32;
                let j = total_height as usize % v.len();
                for i in 0..j {
                    v[i].set_rect(Rect::new(pos, size2(width, height + 1)), border_width);
                    pos.y += height + 1 + border_width;
                }
                for i in j..v.len() {
                    v[i].set_rect(Rect::new(pos, size2(width, height)), border_width);
                    pos.y += height + border_width;
                }
            }
        }
    }

    fn get_rect(&self) -> Rect<u32, PixelSize> {
        match self {
            Node::Leaf(t) => t.get_rect(),
            Node::InnerH(_, r, _) | Node::InnerV(_, r, _) => *r,
        }
    }

    fn active_mut(&mut self) -> &mut TextView {
        match self {
            Node::Leaf(t) => t,
            Node::InnerH(v, _, i) | Node::InnerV(v, _, i) => v[i.unwrap()].active_mut(),
        }
    }

    fn draw(&mut self, active_ctx: &mut ActiveRenderCtx, is_active: bool) {
        match self {
            Node::Leaf(t) => t.draw(active_ctx, is_active),
            Node::InnerH(v, _, i) | Node::InnerV(v, _, i) => {
                for j in 0..v.len() {
                    v[j].draw(
                        active_ctx,
                        if let Some(i) = *i {
                            j == i && is_active
                        } else {
                            false
                        },
                    );
                }
            }
        }
    }
}
