// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::fmt::{Display, Formatter, Result};

#[derive(Debug, Eq, PartialEq)]
pub(in crate::ui) enum GlErrTyp {
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

impl Display for GlErrTyp {
    fn fmt(&self, f: &mut Formatter) -> Result {
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

pub(in crate::ui) fn gl_get_error() -> Option<GlErrTyp> {
    GlErrTyp::from_raw(unsafe { gl::GetError() })
}

macro_rules! gl_error_check {
    () => {
        {
            if let Some(err) = crate::ui::opengl::error::gl_get_error() {
                panic!("OpenGL error: {}", err);
            }
        }
    };
    ($($arg:tt)*) => {
        {
            if let Some(err) = crate::ui::opengl::error::gl_get_error() {
                panic!("OpenGL error: {}: {}", format_args!($($arg)*), err);
            }
        }
    };
}
