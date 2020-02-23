// (C) 2019 Srimanta Barua <srimanta.barua1@gmail.com>

use euclid::{Point2D, Size2D};
use glfw::{Context, Window};

use crate::types::Color;

use super::types::PixelSize;

#[macro_use]
mod error;

/// Placeholder for a GL context that is not being used
#[derive(Clone)]
pub(super) struct Gl;

impl Gl {
    pub(super) fn activate(&self, window: &mut Window) -> ActiveGl {
        window.make_current();
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::PixelStorei(gl::PACK_ALIGNMENT, 1);
        }
        ActiveGl(self)
    }
}

pub(super) struct ActiveGl<'a>(&'a Gl);

impl<'a> ActiveGl<'a> {
    pub(super) fn viewport(
        &mut self,
        point: Point2D<i32, PixelSize>,
        size: Size2D<u32, PixelSize>,
    ) {
        unsafe {
            gl::Viewport(point.x, point.y, size.width as i32, size.height as i32);
        }
    }

    pub(super) fn clear_color(&mut self, color: Color) {
        let (r, g, b, a) = color.to_opengl_color();
        unsafe {
            gl::ClearColor(r, g, b, a);
        }
    }

    pub(super) fn clear(&mut self) {
        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
        }
    }
}
