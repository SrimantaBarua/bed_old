// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::ops::Drop;
use std::ptr;

use euclid::{Rect, Size2D};
use gl::types::{GLenum, GLuint};

use crate::types::{PixelSize, TextureSize};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::ui) enum TexUnit {
    Texture0,
    Texture1,
}

impl TexUnit {
    fn to_gl(&self) -> GLenum {
        match self {
            TexUnit::Texture0 => gl::TEXTURE0,
            TexUnit::Texture1 => gl::TEXTURE1,
        }
    }
}

/// Wrapper around OpenGL textures
pub(in crate::ui) struct GlTexture {
    id: GLuint,
    unit: TexUnit,
    size: Size2D<u32, PixelSize>,
}

impl GlTexture {
    pub(in crate::ui) fn new(unit: TexUnit, size: Size2D<u32, PixelSize>) -> GlTexture {
        let mut id = 0;
        unsafe {
            gl::GenTextures(1, &mut id);
            gl::ActiveTexture(unit.to_gl());
            gl::BindTexture(gl::TEXTURE_2D, id);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RED as i32,
                size.width as i32,
                size.height as i32,
                0,
                gl::RED,
                gl::UNSIGNED_BYTE,
                ptr::null(),
            );
        }
        GlTexture {
            id: id,
            size: size,
            unit: unit,
        }
    }

    /// Activate texture so that it can be used
    pub(in crate::ui) fn activate(&mut self) {
        unsafe {
            gl::ActiveTexture(self.unit.to_gl());
            gl::BindTexture(gl::TEXTURE_2D, self.id);
        }
    }

    /// Deactivate texture
    pub(in crate::ui) fn deactivate(&mut self) {
        unsafe {
            gl::ActiveTexture(self.unit.to_gl());
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
    }

    /// Fill texture sub-image
    pub(in crate::ui) fn sub_image(&mut self, rect: Rect<u32, PixelSize>, data: &[u8]) {
        let max = rect.max();
        assert!(max.x <= self.size.width, "texture coords out of bounds");
        assert!(max.y <= self.size.height, "texture coords out of bounds");
        unsafe {
            gl::ActiveTexture(self.unit.to_gl());
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                rect.origin.x as i32,
                rect.origin.y as i32,
                rect.size.width as i32,
                rect.size.height as i32,
                gl::RED,
                gl::UNSIGNED_BYTE,
                data.as_ptr() as *const _,
            );
        }
    }

    pub(in crate::ui) fn get_tex_dimensions(
        &self,
        rect: Rect<u32, PixelSize>,
    ) -> Rect<f32, TextureSize> {
        let max = rect.max();
        assert!(max.x <= self.size.width, "texture coords out of bounds");
        assert!(max.y <= self.size.height, "texture coords out of bounds");
        rect.cast()
            .cast_unit()
            .scale(1.0 / self.size.width as f32, 1.0 / self.size.height as f32)
    }
}

impl Drop for GlTexture {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &mut self.id);
        }
    }
}
