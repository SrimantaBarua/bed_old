// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::marker::PhantomData;
use std::ops::Drop;
use std::ptr;
use std::rc::Rc;

use super::gl::{
    self,
    types::{GLenum, GLuint},
    GlInner,
};
use euclid::{point2, size2, Rect, Size2D};

use crate::types::{PixelSize, TextureSize};

#[derive(Clone, Copy)]
pub(in crate::ui) enum TexUnit {
    Texture0,
    Texture1,
    Texture2,
}

impl TexUnit {
    fn to_gl(&self) -> GLenum {
        match self {
            TexUnit::Texture0 => gl::TEXTURE0,
            TexUnit::Texture1 => gl::TEXTURE1,
            TexUnit::Texture2 => gl::TEXTURE2,
        }
    }
}

pub(in crate::ui) trait TexFormat {
    fn format() -> GLenum;
}

pub(in crate::ui) struct TexRGB;

impl TexFormat for TexRGB {
    fn format() -> GLenum {
        gl::RGB
    }
}

pub(in crate::ui) struct TexRed;

impl TexFormat for TexRed {
    fn format() -> GLenum {
        gl::RED
    }
}

/// Wrapper around OpenGL textures
pub(in crate::ui) struct GlTexture<F>
where
    F: TexFormat,
{
    pub(super) id: GLuint,
    unit: TexUnit,
    size: Size2D<u32, PixelSize>,
    gl: Rc<GlInner>,
    phantom: PhantomData<F>,
}

impl<F> GlTexture<F>
where
    F: TexFormat,
{
    pub(super) fn new(
        gl: Rc<GlInner>,
        unit: TexUnit,
        size: Size2D<u32, PixelSize>,
    ) -> GlTexture<F> {
        let mut id = 0;
        unsafe {
            gl.GenTextures(1, &mut id);
            gl.ActiveTexture(unit.to_gl());
            gl.BindTexture(gl::TEXTURE_2D, id);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            gl.TexImage2D(
                gl::TEXTURE_2D,
                0,
                F::format() as i32,
                size.width as i32,
                size.height as i32,
                0,
                F::format(),
                gl::UNSIGNED_BYTE,
                ptr::null(),
            );
        }
        GlTexture {
            id: id,
            size: size,
            unit: unit,
            gl,
            phantom: PhantomData,
        }
    }

    /// Activate texture so that it can be used
    pub(in crate::ui) fn activate(&mut self) {
        unsafe {
            self.gl.ActiveTexture(self.unit.to_gl());
            self.gl.BindTexture(gl::TEXTURE_2D, self.id);
        }
    }

    /// Deactivate texture
    pub(in crate::ui) fn deactivate(&mut self) {
        unsafe {
            self.gl.ActiveTexture(self.unit.to_gl());
            self.gl.BindTexture(gl::TEXTURE_2D, 0);
        }
    }

    /// Fill texture sub-image
    pub(in crate::ui) fn sub_image(&mut self, rect: Rect<u32, PixelSize>, data: &[u8]) {
        let max = rect.max();
        assert!(max.x <= self.size.width, "texture coords out of bounds");
        assert!(max.y <= self.size.height, "texture coords out of bounds");
        unsafe {
            self.gl.ActiveTexture(self.unit.to_gl());
            self.gl.TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                rect.origin.x as i32,
                rect.origin.y as i32,
                rect.size.width as i32,
                rect.size.height as i32,
                F::format(),
                gl::UNSIGNED_BYTE,
                data.as_ptr() as *const _,
            );
        }
    }

    pub(in crate::ui) fn get_tex_dimensions(
        &self,
        rect: Rect<i32, PixelSize>,
    ) -> Rect<f32, TextureSize> {
        let qbox = rect.to_box2d();
        Rect::new(
            point2(
                qbox.min.x as f32 / self.size.width as f32,
                1.0 - (qbox.min.y as f32 / self.size.height as f32),
            ),
            size2(
                rect.size.width as f32 / self.size.width as f32,
                -(rect.size.height as f32) / self.size.height as f32,
            ),
        )
    }

    pub(in crate::ui) fn get_inverted_tex_dimension(
        &self,
        rect: Rect<i32, PixelSize>,
    ) -> Rect<f32, TextureSize> {
        rect.cast()
            .cast_unit()
            .scale(1.0 / self.size.width as f32, 1.0 / self.size.height as f32)
    }
}

impl<F> Drop for GlTexture<F>
where
    F: TexFormat,
{
    fn drop(&mut self) {
        unsafe {
            self.gl.DeleteTextures(1, &mut self.id);
        }
    }
}
