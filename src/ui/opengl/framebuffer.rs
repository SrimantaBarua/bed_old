// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::ops::Drop;
use std::rc::Rc;

use super::gl::{self, types::GLuint, GlInner};
use euclid::Size2D;

use super::texture::{GlTexture, TexRGB, TexUnit};
use crate::types::PixelSize;

pub(in crate::ui) struct Framebuffer {
    gl: Rc<GlInner>,
    tex: GlTexture<TexRGB>,
    rbo: Renderbuffer,
    fbo: GLuint,
    unit: TexUnit,
}

impl Framebuffer {
    pub(super) fn new(gl: Rc<GlInner>, unit: TexUnit, size: Size2D<u32, PixelSize>) -> Framebuffer {
        let mut fbo = 0;
        unsafe {
            gl.GenFramebuffers(1, &mut fbo);
            gl.BindFramebuffer(gl::FRAMEBUFFER, fbo);

            let tex = GlTexture::new(gl.clone(), unit, size);
            gl.FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                tex.id,
                0,
            );

            let rbo = Renderbuffer::new(gl.clone(), size);
            gl.FramebufferRenderbuffer(
                gl::FRAMEBUFFER,
                gl::DEPTH_STENCIL_ATTACHMENT,
                gl::RENDERBUFFER,
                rbo.id,
            );

            if gl.CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
                panic!("framebuffer is not complete");
            }
            let mut ret = Framebuffer {
                tex: tex,
                rbo: rbo,
                fbo: fbo,
                unit: unit,
                gl: gl,
            };
            ret.unbind();
            ret
        }
    }

    pub(in crate::ui) fn resize(&mut self, size: Size2D<u32, PixelSize>) {
        unsafe {
            self.gl.BindFramebuffer(gl::FRAMEBUFFER, self.fbo);

            self.tex = GlTexture::new(self.gl.clone(), self.unit, size);
            self.gl.FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                self.tex.id,
                0,
            );

            self.rbo = Renderbuffer::new(self.gl.clone(), size);
            self.gl.FramebufferRenderbuffer(
                gl::FRAMEBUFFER,
                gl::DEPTH_STENCIL_ATTACHMENT,
                gl::RENDERBUFFER,
                self.rbo.id,
            );

            if self.gl.CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
                panic!("framebuffer is not complete");
            }
            self.unbind();
        }
    }

    pub(in crate::ui) fn bind(&mut self) {
        unsafe {
            self.gl.BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
        }
    }

    pub(in crate::ui) fn bind_texture(&mut self) {
        self.tex.activate();
    }

    pub(in crate::ui) fn get_texture(&self) -> &GlTexture<TexRGB> {
        &self.tex
    }

    pub(in crate::ui) fn unbind(&mut self) {
        unsafe {
            self.gl.BindFramebuffer(gl::FRAMEBUFFER, 0);
        }
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            self.gl.DeleteFramebuffers(1, &mut self.fbo);
        }
    }
}

struct Renderbuffer {
    id: GLuint,
    gl: Rc<GlInner>,
}

impl Renderbuffer {
    fn new(gl: Rc<GlInner>, size: Size2D<u32, PixelSize>) -> Renderbuffer {
        let mut rbo = 0;
        unsafe {
            gl.GenRenderbuffers(1, &mut rbo);
            gl.BindRenderbuffer(gl::RENDERBUFFER, rbo);
            gl.RenderbufferStorage(
                gl::RENDERBUFFER,
                gl::DEPTH24_STENCIL8,
                size.width as i32,
                size.height as i32,
            );
        }
        Renderbuffer { id: rbo, gl: gl }
    }
}

impl Drop for Renderbuffer {
    fn drop(&mut self) {
        unsafe {
            self.gl.DeleteRenderbuffers(1, &mut self.id);
        }
    }
}
