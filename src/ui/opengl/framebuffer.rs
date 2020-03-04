// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::ops::Drop;

use euclid::Size2D;
use gl::types::GLuint;

use super::texture::{GlTexture, TexRGB, TexUnit};
use crate::types::PixelSize;

pub(in crate::ui) struct Framebuffer {
    tex: GlTexture<TexRGB>,
    rbo: Renderbuffer,
    fbo: GLuint,
}

impl Framebuffer {
    pub(in crate::ui) fn new(unit: TexUnit, size: Size2D<u32, PixelSize>) -> Framebuffer {
        let mut fbo = 0;
        unsafe {
            gl::GenFramebuffers(1, &mut fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);

            let tex = GlTexture::new(unit, size);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                tex.id,
                0,
            );

            let rbo = Renderbuffer::new(size);
            gl::FramebufferRenderbuffer(
                gl::FRAMEBUFFER,
                gl::DEPTH_STENCIL_ATTACHMENT,
                gl::RENDERBUFFER,
                rbo.id,
            );

            if gl::CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
                panic!("framebuffer is not complete");
            }
            let mut ret = Framebuffer {
                tex: tex,
                rbo: rbo,
                fbo: fbo,
            };
            ret.unbind();
            ret
        }
    }

    pub(in crate::ui) fn unbind(&mut self) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        }
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &mut self.fbo);
        }
    }
}

struct Renderbuffer {
    id: GLuint,
}

impl Renderbuffer {
    fn new(size: Size2D<u32, PixelSize>) -> Renderbuffer {
        let mut rbo = 0;
        unsafe {
            gl::GenRenderbuffers(1, &mut rbo);
            gl::BindRenderbuffer(gl::RENDERBUFFER, rbo);
            gl::RenderbufferStorage(
                gl::RENDERBUFFER,
                gl::DEPTH24_STENCIL8,
                size.width as i32,
                size.height as i32,
            );
        }
        Renderbuffer { id: rbo }
    }
}

impl Drop for Renderbuffer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteRenderbuffers(1, &mut self.id);
        }
    }
}
