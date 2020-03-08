// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::ffi::CStr;
use std::ops::Drop;

use euclid::{point2, Point2D, Rect, SideOffsets2D, Size2D};

use super::font::{FaceKey, RasterFace};
use super::glyphrender::{ActiveGlyphRenderer, GlyphRenderer};
use super::opengl::{ActiveGl, ElemArr, Framebuffer, Gl, Mat4, ShaderProgram, TexUnit};
use super::quad::{ColorQuad, TexColorQuad, TexQuad};
use crate::types::{Color, PixelSize, TextSize, TextStyle, DPI};

pub(super) struct RenderCtx {
    gl: Gl,
    projection_matrix: Mat4,
    size: Size2D<u32, PixelSize>,
    pub(super) dpi: Size2D<u32, DPI>,
    clear_color: Color,
    glyph_renderer: GlyphRenderer,
    // Framebuffers
    framebuffers: [Framebuffer; 1],
    // shaders
    clr_quad_shader: ShaderProgram,
    tex_clr_quad_shader: ShaderProgram,
    shadow_shader: ShaderProgram,
    // arrays
    clr_quad_arr: ElemArr<ColorQuad>,
    tex_clr_quad_arr: ElemArr<TexColorQuad>,
    tex_quad_arr: ElemArr<TexQuad>,
}

impl RenderCtx {
    pub(super) fn new(
        size: Size2D<u32, PixelSize>,
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
        let shadow_vsrc = include_str!("opengl/shader_src/shadow.vert");
        let shadow_fsrc = include_str!("opengl/shader_src/shadow.frag");
        let shadow_shader =
            ShaderProgram::new(shadow_vsrc, shadow_fsrc).expect("failed to compile shader");
        RenderCtx {
            gl: Gl,
            projection_matrix: Mat4::projection(size.cast()),
            size: size,
            dpi: dpi,
            clear_color: clear_color,
            glyph_renderer: GlyphRenderer::new(dpi),
            clr_quad_shader: clr_shader,
            tex_clr_quad_shader: tex_clr_shader,
            shadow_shader: shadow_shader,
            clr_quad_arr: ElemArr::new(64),
            tex_clr_quad_arr: ElemArr::new(4096),
            tex_quad_arr: ElemArr::new(8),
            framebuffers: [Framebuffer::new(TexUnit::Texture1, size)],
        }
    }

    pub(super) fn activate(&mut self, window: &mut glfw::Window) -> ActiveRenderCtx {
        let mut active_gl = self.gl.activate(window);
        active_gl.viewport(Rect::new(point2(0, 0), self.size.cast()));
        self.framebuffers[0].bind_texture();
        let mut ret = ActiveRenderCtx {
            active_gl: active_gl,
            size: self.size,
            projection_matrix: &self.projection_matrix,
            dpi: self.dpi,
            clear_color: self.clear_color,
            clr_quad_shader: &mut self.clr_quad_shader,
            tex_clr_quad_shader: &mut self.tex_clr_quad_shader,
            shadow_shader: &mut self.shadow_shader,
            tex_quad_arr: &mut self.tex_quad_arr,
            clr_quad_arr: &mut self.clr_quad_arr,
            framebuffers: &mut self.framebuffers,
            active_glyph_renderer: self.glyph_renderer.activate(&mut self.tex_clr_quad_arr),
        };
        ret.set_uniforms();
        ret
    }

    pub(super) fn set_size(&mut self, size: Size2D<u32, PixelSize>) {
        self.size = size;
        self.projection_matrix = Mat4::projection(size);
        self.framebuffers[0].resize(size);
    }
}

pub(super) struct ActiveRenderCtx<'a> {
    active_gl: ActiveGl<'a>,
    size: Size2D<u32, PixelSize>,
    projection_matrix: &'a Mat4,
    clear_color: Color,
    dpi: Size2D<u32, DPI>,
    active_glyph_renderer: ActiveGlyphRenderer<'a, 'a>,
    // framebuffers
    framebuffers: &'a mut [Framebuffer; 1],
    // shaders
    clr_quad_shader: &'a mut ShaderProgram,
    tex_clr_quad_shader: &'a mut ShaderProgram,
    shadow_shader: &'a mut ShaderProgram,
    // arrays
    clr_quad_arr: &'a mut ElemArr<ColorQuad>,
    tex_quad_arr: &'a mut ElemArr<TexQuad>,
}

impl<'a> ActiveRenderCtx<'a> {
    pub(super) fn clear(&mut self) {
        self.active_gl.clear_color(self.clear_color);
        self.active_gl.clear();

        self.framebuffers[0].bind();
        self.active_gl.clear_color(Color::new(0, 0, 0, 255));
        self.active_gl.clear();
        self.framebuffers[0].unbind();
    }

    pub(super) fn get_widget_context<'b>(
        &'b mut self,
        rect: Rect<i32, PixelSize>,
        background_color: Color,
    ) -> WidgetRenderCtx<'b, 'a> {
        let mut ret = WidgetRenderCtx {
            active_ctx: self,
            rect: rect,
            background_color: background_color,
        };
        ret.draw_bg_stencil();
        ret
    }

    pub(super) fn draw_shadow(&mut self, rect: Rect<i32, PixelSize>) {
        let outer_dims = SideOffsets2D::new(5, 5, 5, 5);
        let outer_rect = rect.outer_rect(outer_dims);

        self.active_gl.set_stencil_test(false);
        self.framebuffers[0].bind();
        {
            let active_shader = self.clr_quad_shader.use_program(&mut self.active_gl);
            self.clr_quad_arr
                .push(ColorQuad::new(rect.cast(), Color::new(255, 0, 0, 255)));
            self.clr_quad_arr.flush(&active_shader);
        }
        self.framebuffers[0].unbind();
        {
            let tex = self.framebuffers[0].get_texture();
            let active_shader = self.shadow_shader.use_program(&mut self.active_gl);
            let trect = tex.get_tex_dimensions(outer_rect);
            let quad = TexQuad::new(outer_rect.cast(), trect);
            self.tex_quad_arr.push(quad);
            self.tex_quad_arr.flush(&active_shader);
        }
    }

    fn set_uniforms(&mut self) {
        let projection = CStr::from_bytes_with_nul(b"projection\0").unwrap();
        let text = CStr::from_bytes_with_nul(b"text\0").unwrap();
        let tex = CStr::from_bytes_with_nul(b"tex\0").unwrap();
        {
            let mut active_shader = self.clr_quad_shader.use_program(&mut self.active_gl);
            active_shader.uniform_mat4f(&projection, &self.projection_matrix);
        }
        {
            let mut active_shader = self.tex_clr_quad_shader.use_program(&mut self.active_gl);
            active_shader.uniform_mat4f(&projection, &self.projection_matrix);
            active_shader.uniform_1i(&text, 0);
        }
        {
            let mut active_shader = self.shadow_shader.use_program(&mut self.active_gl);
            active_shader.uniform_mat4f(&projection, &self.projection_matrix);
            active_shader.uniform_1i(&tex, 1);
        }
    }
}

pub(super) struct WidgetRenderCtx<'a, 'b> {
    active_ctx: &'a mut ActiveRenderCtx<'b>,
    rect: Rect<i32, PixelSize>,
    background_color: Color,
}

impl<'a, 'b> WidgetRenderCtx<'a, 'b> {
    pub(super) fn color_quad(&mut self, rect: Rect<i32, PixelSize>, color: Color) {
        let tvec = self.rect.origin.to_vector();
        self.active_ctx
            .clr_quad_arr
            .push(ColorQuad::new(rect.translate(tvec).cast(), color));
    }

    pub(super) fn glyph(
        &mut self,
        pos: Point2D<i32, PixelSize>,
        face: FaceKey,
        gid: u32,
        size: TextSize,
        color: Color,
        style: TextStyle,
        raster: &mut RasterFace,
    ) {
        let tvec = self.rect.origin.to_vector();
        let pos = pos + tvec;
        self.active_ctx
            .active_glyph_renderer
            .render_glyph(pos, face, gid, size, color, style, raster);
    }

    pub(super) fn flush(&mut self) {
        {
            let active_shader = self
                .active_ctx
                .clr_quad_shader
                .use_program(&mut self.active_ctx.active_gl);
            self.active_ctx.clr_quad_arr.flush(&active_shader);
        }
        {
            let active_shader = self
                .active_ctx
                .tex_clr_quad_shader
                .use_program(&mut self.active_ctx.active_gl);
            self.active_ctx.active_glyph_renderer.flush(&active_shader);
        }
    }

    fn draw_bg_stencil(&mut self) {
        // Activate stencil writing
        self.active_ctx.active_gl.set_stencil_test(true);
        self.active_ctx.active_gl.set_stencil_writing();
        // Draw background and write to stencil
        {
            let active_shader = self
                .active_ctx
                .clr_quad_shader
                .use_program(&mut self.active_ctx.active_gl);
            self.active_ctx
                .clr_quad_arr
                .push(ColorQuad::new(self.rect.cast(), self.background_color));
            self.active_ctx.clr_quad_arr.flush(&active_shader);
        }
        self.active_ctx.active_gl.set_stencil_reading();
    }
}

impl<'a, 'b> Drop for WidgetRenderCtx<'a, 'b> {
    fn drop(&mut self) {
        self.flush();
        self.active_ctx.active_gl.clear_stencil();
    }
}
