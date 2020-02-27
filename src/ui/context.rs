// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::ffi::CStr;
use std::ops::Drop;

use euclid::{Rect, Size2D};

use super::opengl::{ActiveGl, ElemArr, Gl, Mat4, ShaderProgram};
use super::quad::{ColorQuad, TexColorQuad};
use super::types::{PixelSize, TextureSize, DPI};
use crate::types::Color;

pub(super) struct RenderCtx {
    gl: Gl,
    projection_matrix: Mat4,
    rect: Rect<i32, PixelSize>,
    dpi: Size2D<u32, DPI>,
    clear_color: Color,
    clr_quad_shader: ShaderProgram,
    tex_clr_quad_shader: ShaderProgram,
    clr_quad_arr: ElemArr<ColorQuad>,
    tex_clr_quad_arr: ElemArr<TexColorQuad>,
}

impl RenderCtx {
    pub(super) fn new(
        rect: Rect<i32, PixelSize>,
        dpi: Size2D<u32, DPI>,
        clear_color: Color,
    ) -> RenderCtx {
        // Compile and link shaders
        let clr_vsrc = include_str!("opengl/shader_src/colored_quad.vert");
        let clr_fsrc = include_str!("opengl/shader_src/colored_quad.frag");
        let clr_shader = ShaderProgram::new(clr_vsrc, clr_fsrc).expect("failed to compile shader");
        let tex_clr_vsrc = include_str!("opengl/shader_src/tex_color_quad.vert");
        let tex_clr_fsrc = include_str!("opengl/shader_src/tex_color_quad.frag");
        let tex_clr_shader =
            ShaderProgram::new(tex_clr_vsrc, tex_clr_fsrc).expect("failed to compile shader");
        RenderCtx {
            gl: Gl,
            projection_matrix: Mat4::projection(rect.size.cast()),
            rect: rect,
            dpi: dpi,
            clear_color: clear_color,
            clr_quad_shader: clr_shader,
            tex_clr_quad_shader: tex_clr_shader,
            clr_quad_arr: ElemArr::new(64),
            tex_clr_quad_arr: ElemArr::new(4096),
        }
    }

    pub(super) fn activate(&mut self, window: &mut glfw::Window) -> ActiveRenderCtx {
        let mut active_gl = self.gl.activate(window);
        active_gl.viewport(self.rect);
        ActiveRenderCtx {
            active_gl: active_gl,
            projection_matrix: &self.projection_matrix,
            clear_color: self.clear_color,
            clr_quad_shader: &mut self.clr_quad_shader,
            tex_clr_quad_shader: &mut self.tex_clr_quad_shader,
            clr_quad_arr: &mut self.clr_quad_arr,
            tex_clr_quad_arr: &mut self.tex_clr_quad_arr,
        }
    }

    pub(super) fn set_size(&mut self, size: Size2D<u32, PixelSize>) {
        self.rect.size = size.cast();
        self.projection_matrix = Mat4::projection(size);
    }
}

pub(super) struct ActiveRenderCtx<'a> {
    active_gl: ActiveGl<'a>,
    projection_matrix: &'a Mat4,
    clear_color: Color,
    clr_quad_shader: &'a mut ShaderProgram,
    tex_clr_quad_shader: &'a mut ShaderProgram,
    clr_quad_arr: &'a mut ElemArr<ColorQuad>,
    tex_clr_quad_arr: &'a mut ElemArr<TexColorQuad>,
}

impl<'a> ActiveRenderCtx<'a> {
    pub(super) fn clear(&mut self) {
        self.active_gl.clear_color(self.clear_color);
        self.active_gl.clear();
    }

    pub(super) fn color_quad(&mut self, rect: Rect<i32, PixelSize>, color: Color) {
        self.clr_quad_arr.push(ColorQuad::new(rect.cast(), color));
    }

    pub(super) fn tex_color_quad(
        &mut self,
        quad: Rect<i32, PixelSize>,
        texture: Rect<f32, TextureSize>,
        color: Color,
    ) {
        self.tex_clr_quad_arr
            .push(TexColorQuad::new(quad.cast(), texture, color));
    }

    pub(super) fn flush(&mut self) {
        let name = CStr::from_bytes_with_nul(b"projection\0").unwrap();
        {
            let mut active_shader = self.clr_quad_shader.use_program(&mut self.active_gl);
            active_shader.uniform_mat4f(&name, &self.projection_matrix);
            self.clr_quad_arr.flush(&active_shader);
        }
        {
            let mut active_shader = self.tex_clr_quad_shader.use_program(&mut self.active_gl);
            active_shader.uniform_mat4f(&name, &self.projection_matrix);
            self.tex_clr_quad_arr.flush(&active_shader);
        }
    }
}

impl<'a> Drop for ActiveRenderCtx<'a> {
    fn drop(&mut self) {
        self.flush();
    }
}
