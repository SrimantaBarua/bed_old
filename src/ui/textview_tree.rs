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
            config,
            dpi,
            line_numbers,
            relative_number,
            view_id,
        );
        TextViewTree { root: leaf }
    }

    pub(super) fn draw(&mut self, active_ctx: &mut ActiveRenderCtx) {
        self.root.draw(active_ctx)
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
        self.root.set_rect(rect);
    }

    pub(super) fn active_mut(&mut self) -> &mut TextView {
        self.root.active_mut()
    }

    pub(super) fn split_h(&mut self, view_id: usize) {
        self.root.split_h(view_id);
        self.root.compute_rects();
    }

    pub(super) fn split_v(&mut self, view_id: usize) {
        self.root.split_v(view_id);
        self.root.compute_rects();
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

    fn move_cursor_to_point(&mut self, mut point: (i32, i32)) {
        match self {
            Node::Leaf(t) => t.move_cursor_to_point(point),
            Node::InnerH(v, _, i) => {
                for j in 0..v.len() {
                    let width = v[j].get_rect().size.width as i32;
                    if point.0 < width {
                        v[j].move_cursor_to_point(point);
                        *i = Some(j);
                        break;
                    } else {
                        point.0 -= width;
                    }
                }
            }
            Node::InnerV(v, _, i) => {
                for j in 0..v.len() {
                    let height = v[j].get_rect().size.height as i32;
                    if point.1 < height {
                        v[j].move_cursor_to_point(point);
                        *i = Some(j);
                        break;
                    } else {
                        point.1 -= height;
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
                    let width = v[j].get_rect().size.width as i32;
                    if let Some(c) = cursor {
                        if c.0 < width {
                            ret |= v[j].scroll(cursor, force, time);
                            cursor = None;
                        } else {
                            cursor = Some((c.0 - width, c.1));
                            ret |= v[j].scroll(None, (0.0, 0.0), time);
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
                    let height = v[j].get_rect().size.height as i32;
                    if let Some(c) = cursor {
                        if c.1 < height {
                            ret |= v[j].scroll(cursor, force, time);
                            cursor = None;
                        } else {
                            cursor = Some((c.0, c.1 - height));
                            ret |= v[j].scroll(None, (0.0, 0.0), time);
                        }
                    } else {
                        ret |= v[j].scroll(None, (0.0, 0.0), time);
                    };
                }
                ret
            }
        }
    }

    fn set_rect(&mut self, rect: Rect<u32, PixelSize>) {
        match self {
            Node::Leaf(t) => t.set_rect(rect),
            Node::InnerH(_, r, _) | Node::InnerV(_, r, _) => *r = rect,
        }
        self.compute_rects();
    }

    fn compute_rects(&mut self) {
        match self {
            Node::Leaf(_) => {}
            Node::InnerH(v, r, _) => {
                let mut pos = r.origin;
                let height = r.size.height;
                let width = r.size.width / v.len() as u32;
                let j = r.size.width as usize % v.len();
                for i in 0..j {
                    v[i].set_rect(Rect::new(pos, size2(width + 1, height)));
                    pos.x += width + 1;
                }
                for i in j..v.len() {
                    v[i].set_rect(Rect::new(pos, size2(width, height)));
                    pos.x += width;
                }
            }
            Node::InnerV(v, r, _) => {
                let mut pos = r.origin;
                let height = r.size.height / v.len() as u32;
                let width = r.size.width;
                let j = r.size.height as usize % v.len();
                for i in 0..j {
                    v[i].set_rect(Rect::new(pos, size2(width, height + 1)));
                    pos.y += height + 1;
                }
                for i in j..v.len() {
                    v[i].set_rect(Rect::new(pos, size2(width, height)));
                    pos.y += height;
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

    fn draw(&mut self, active_ctx: &mut ActiveRenderCtx) {
        match self {
            Node::Leaf(t) => t.draw(active_ctx),
            Node::InnerH(v, _, _) | Node::InnerV(v, _, _) => {
                for i in 0..v.len() {
                    v[i].draw(active_ctx);
                }
            }
        }
    }
}
