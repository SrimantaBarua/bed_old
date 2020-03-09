// (C) 2019 Srimanta Barua <srimanta.barua1@gmail.com>

use std::fmt;
use std::rc::Rc;

use euclid::{Rect, Size2D};
use glfw::Window;

use crate::types::{Color, PixelSize};

mod framebuffer;
mod shader;
mod texture;
mod vert_array;

pub(super) use framebuffer::Framebuffer;
pub(super) use shader::{ActiveShaderProgram, ShaderProgram};
pub(super) use texture::{GlTexture, TexRGB, TexRed, TexUnit};
pub(super) use vert_array::{ElemArr, Element};

mod gl {
    pub(super) use self::Gles2 as GlInner;
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

/// Placeholder for a GL context that is not being used
#[derive(Clone)]
pub(super) struct Gl {
    gl: Rc<gl::GlInner>,
}

impl Gl {
    pub(super) fn load(window: &mut Window) -> Gl {
        let gl = Rc::new(gl::GlInner::load_with(|s| window.get_proc_address(s)));
        unsafe {
            gl.Enable(gl::BLEND);
            gl.BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl.ActiveTexture(gl::TEXTURE0);
            gl.PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl.PixelStorei(gl::PACK_ALIGNMENT, 1);
        }
        Gl { gl }
    }

    pub(super) fn viewport(&mut self, rect: Rect<i32, PixelSize>) {
        unsafe {
            self.gl.Viewport(
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
            self.gl.ClearColor(r, g, b, a);
        }
    }

    pub(super) fn clear(&mut self) {
        unsafe {
            self.gl.Clear(gl::COLOR_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
        }
    }

    pub(super) fn set_stencil_test(&mut self, val: bool) {
        if val {
            unsafe {
                self.gl.Enable(gl::STENCIL_TEST);
                self.gl.StencilOp(gl::ZERO, gl::ZERO, gl::REPLACE);
            }
        } else {
            unsafe {
                self.gl.Disable(gl::STENCIL_TEST);
            }
        }
    }

    pub(super) fn set_stencil_writing(&mut self) {
        unsafe {
            self.gl.StencilFunc(gl::ALWAYS, 1, 0xff);
            self.gl.StencilMask(0xff);
        }
    }

    pub(super) fn set_stencil_reading(&mut self) {
        unsafe {
            self.gl.StencilFunc(gl::EQUAL, 1, 0xff);
            self.gl.StencilMask(0x00);
        }
    }

    pub(super) fn clear_stencil(&mut self) {
        unsafe {
            self.gl.StencilMask(0xff);
            self.gl.Clear(gl::STENCIL_BUFFER_BIT);
            self.gl.StencilMask(0x00);
        }
    }

    pub(super) fn new_elem_arr<E>(&mut self, cap: usize) -> ElemArr<E>
    where
        E: Element,
    {
        ElemArr::new(self.gl.clone(), cap)
    }

    pub(super) fn new_shader(&mut self, vsrc: &str, fsrc: &str) -> Result<ShaderProgram, String> {
        ShaderProgram::new(self.gl.clone(), vsrc, fsrc)
    }

    pub(super) fn use_shader<'a, 'b>(
        &'a mut self,
        shader: &'b mut ShaderProgram,
    ) -> ActiveShaderProgram<'a, 'b> {
        shader.use_program(self)
    }

    pub(super) fn new_texture<T>(
        &mut self,
        unit: TexUnit,
        size: Size2D<u32, PixelSize>,
    ) -> GlTexture<T>
    where
        T: texture::TexFormat,
    {
        GlTexture::new(self.gl.clone(), unit, size)
    }

    pub(super) fn new_framebuffer(
        &mut self,
        unit: TexUnit,
        size: Size2D<u32, PixelSize>,
    ) -> Framebuffer {
        Framebuffer::new(self.gl.clone(), unit, size)
    }

    fn get_error(&mut self) -> Option<GlErrTyp> {
        GlErrTyp::from_raw(unsafe { self.gl.GetError() })
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

#[derive(Debug, Eq, PartialEq)]
pub(super) enum GlErrTyp {
    InvalidEnum,
    InvalidValue,
    InvalidOperation,
    StackOverflow,
    StackUnderflow,
    OutOfMemory,
    InvalidFramebufferOperation,
}

impl GlErrTyp {
    fn from_raw(raw: u32) -> Option<GlErrTyp> {
        match raw {
            gl::INVALID_ENUM => Some(GlErrTyp::InvalidEnum),
            gl::INVALID_VALUE => Some(GlErrTyp::InvalidValue),
            gl::INVALID_OPERATION => Some(GlErrTyp::InvalidOperation),
            gl::STACK_OVERFLOW => Some(GlErrTyp::StackOverflow),
            gl::STACK_UNDERFLOW => Some(GlErrTyp::StackUnderflow),
            gl::OUT_OF_MEMORY => Some(GlErrTyp::OutOfMemory),
            gl::INVALID_FRAMEBUFFER_OPERATION => Some(GlErrTyp::InvalidFramebufferOperation),
            _ => None,
        }
    }
}

impl fmt::Display for GlErrTyp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GlErrTyp::InvalidEnum => write!(f, "invalid enum"),
            GlErrTyp::InvalidValue => write!(f, "invalid value"),
            GlErrTyp::InvalidOperation => write!(f, "invalid operation"),
            GlErrTyp::StackOverflow => write!(f, "stack overflow"),
            GlErrTyp::StackUnderflow => write!(f, "stack underflow"),
            GlErrTyp::OutOfMemory => write!(f, "out of memory"),
            GlErrTyp::InvalidFramebufferOperation => write!(f, "invalid framebuffer operation"),
        }
    }
}

macro_rules! gl_error_check {
    (gl:tt) => {
        {
            if let Some(err) = gl.get_error() {
                panic!("OpenGL error: {}", err);
            }
        }
    };
    (gl:tt, $($arg:tt)*) => {
        {
            if let Some(err) = gl.get_error() {
                panic!("OpenGL error: {}: {}", format_args!($($arg)*), err);
            }
        }
    };
}
