// (C) 2019 Srimanta Barua <srimanta.barua1@gmail.com>

use std::fmt;

use euclid::{Rect, Size2D};
use glfw::{Context, Window};

use crate::types::Color;

use super::types::PixelSize;

#[macro_use]
mod error;
mod shader;
mod vert_array;

pub(super) use shader::ShaderProgram;
pub(super) use vert_array::{ElemArr, Element};

/// Placeholder for a GL context that is not being used
#[derive(Clone)]
pub(super) struct Gl;

impl Gl {
    pub(super) fn activate(&mut self, window: &mut Window) -> ActiveGl {
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
    pub(super) fn viewport(&mut self, rect: Rect<i32, PixelSize>) {
        unsafe {
            gl::Viewport(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
            );
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

    pub(super) fn set_stencil_test(&mut self, val: bool) {
        if val {
            unsafe {
                gl::Enable(gl::STENCIL_TEST);
                gl::StencilOp(gl::KEEP, gl::KEEP, gl::REPLACE);
            }
        } else {
            unsafe {
                gl::Disable(gl::STENCIL_TEST);
            }
        }
    }

    pub(super) fn set_stencil_writing(&mut self) {
        unsafe {
            gl::StencilFunc(gl::ALWAYS, 1, 0xff);
            gl::StencilMask(0xff);
        }
    }

    pub(super) fn set_stencil_reading(&mut self) {
        unsafe {
            gl::StencilFunc(gl::EQUAL, 1, 0xff);
            gl::StencilMask(0x00);
        }
    }
}

/// 4x4 Matrix
pub(super) struct Mat4([f32; 16]);

impl Mat4 {
    /// Orthogonal projection matrix from given window dimensions
    #[rustfmt::skip]
    pub(super) fn projection(size: Size2D<u32, PixelSize>) -> Mat4 {
        let (x, y) = (size.width as f32, size.height as f32);
        Mat4(
            [
                2.0 / x, 0.0     , 0.0, 0.0,
                0.0    , -2.0 / y, 0.0, 0.0,
                0.0    , 0.0     , 0.0, 0.0,
                -1.0   , 1.0     , 0.0, 1.0,
            ]
        )
    }

    /// Get pointer
    fn as_ptr(&self) -> *const f32 {
        self.0.as_ptr()
    }
}

impl fmt::Display for Mat4 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "[ {:16} {:16} {:16} {:16} ]",
            self.0[0], self.0[4], self.0[8], self.0[12],
        )?;
        writeln!(
            f,
            "[ {:16} {:16} {:16} {:16} ]",
            self.0[1], self.0[5], self.0[9], self.0[13],
        )?;
        writeln!(
            f,
            "[ {:16} {:16} {:16} {:16} ]",
            self.0[2], self.0[6], self.0[10], self.0[14],
        )?;
        writeln!(
            f,
            "[ {:16} {:16} {:16} {:16} ]",
            self.0[3], self.0[7], self.0[11], self.0[15]
        )
    }
}
