// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use euclid::Rect;

use super::opengl::Element;
use crate::types::{Color, PixelSize, TextureSize};

pub(in crate::ui) struct ColorQuad([f32; 24]);

impl Element for ColorQuad {
    fn num_points_per_vertex() -> usize {
        6
    }

    fn vertex_attributes() -> &'static [(i32, usize, usize)] {
        &[(2, 6, 0), (4, 6, 2)]
    }

    fn data(&self) -> &[f32] {
        &self.0
    }
}

impl ColorQuad {
    #[rustfmt::skip]
    pub(in crate::ui) fn new(
        rect: Rect<f32, PixelSize>,
        color: Color,
    ) -> ColorQuad {
        let (r, g, b, a) = color.to_opengl_color();
        let qbox = rect.to_box2d();
        ColorQuad([
            qbox.min.x, qbox.min.y, r, g, b, a,
            qbox.min.x, qbox.max.y, r, g, b, a,
            qbox.max.x, qbox.min.y, r, g, b, a,
            qbox.max.x, qbox.max.y, r, g, b, a,
        ])
    }
}

pub(in crate::ui) struct TexQuad {
    data: [f32; 16],
}

impl Element for TexQuad {
    fn num_points_per_vertex() -> usize {
        4
    }

    fn vertex_attributes() -> &'static [(i32, usize, usize)] {
        &[(4, 4, 0)]
    }

    fn data(&self) -> &[f32] {
        &self.data
    }
}

impl TexQuad {
    #[rustfmt::skip]
    pub(in crate::ui) fn new(
        quad_rect: Rect<f32, PixelSize>,
        tex_rect: Rect<f32, TextureSize>,
    ) -> TexQuad {
        let qbox = quad_rect.to_box2d();
        let tbox = tex_rect.to_box2d();
        TexQuad {
            data: [
                qbox.min.x, qbox.min.y, tbox.min.x, tbox.min.y,
                qbox.min.x, qbox.max.y, tbox.min.x, tbox.max.y,
                qbox.max.x, qbox.min.y, tbox.max.x, tbox.min.y,
                qbox.max.x, qbox.max.y, tbox.max.x, tbox.max.y,
            ],
        }
    }
}

pub(in crate::ui) struct TexColorQuad {
    data: [f32; 32],
}

impl Element for TexColorQuad {
    fn num_points_per_vertex() -> usize {
        8
    }

    fn vertex_attributes() -> &'static [(i32, usize, usize)] {
        &[(4, 8, 0), (4, 8, 4)]
    }

    fn data(&self) -> &[f32] {
        &self.data
    }
}

impl TexColorQuad {
    #[rustfmt::skip]
    pub(in crate::ui) fn new(
        quad_rect: Rect<f32, PixelSize>,
        tex_rect: Rect<f32, TextureSize>,
        color: Color,
    ) -> TexColorQuad {
        let (r, g, b, a) = color.to_opengl_color();
        let qbox = quad_rect.to_box2d();
        let tbox = tex_rect.to_box2d();
        TexColorQuad {
            data: [
                qbox.min.x, qbox.min.y, tbox.min.x, tbox.min.y, r, g, b, a,
                qbox.min.x, qbox.max.y, tbox.min.x, tbox.max.y, r, g, b, a,
                qbox.max.x, qbox.min.y, tbox.max.x, tbox.min.y, r, g, b, a,
                qbox.max.x, qbox.max.y, tbox.max.x, tbox.max.y, r, g, b, a,
            ],
        }
    }
}
