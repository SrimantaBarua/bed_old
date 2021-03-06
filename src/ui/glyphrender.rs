// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::hash::{Hash, Hasher};

use euclid::{point2, size2, Point2D, Rect, Size2D};
use fnv::FnvHashMap;
use guillotiere::{AllocId, AllocatorOptions, AtlasAllocator};

use crate::font::{FaceKey, RasterFace};
use crate::types::{Color, PixelSize, TextSize, TextStyle, DPI};

use super::opengl::{ActiveShaderProgram, ElemArr, Gl, GlTexture, TexRed, TexUnit};
use super::quad::TexColorQuad;

const GL_TEX_SIZE: u32 = 4096;

/// Uniquely identify a glyph in a face, for a given size
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlyphKey {
    gid: u32,         // Glyph ID
    size: TextSize,   // Point size of text
    face: FaceKey,    // Face to render with
    style: TextStyle, // Text properties (weight, slant)
}

impl Hash for GlyphKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let val = ((self.style.ival() as u64) << 56) // since max gid is 0x10ffff
            | ((self.gid as u64) << 32)
            | ((self.size.ival() as u64) << 16)
            | (self.face.ival() as u64);
        val.hash(state)
    }
}

/// Information about a rendered glyph
#[derive(Debug)]
struct RenderedGlyph {
    bearing: Size2D<i32, PixelSize>, // Glyph bearing (left, top)
    rect: Rect<u32, PixelSize>,      // Glyph bounding rectangle
    alloc: AllocId,                  // Allocation ID
}

impl RenderedGlyph {
    fn new(
        rect: Rect<u32, PixelSize>,
        bearing: Size2D<i32, PixelSize>,
        alloc: AllocId,
        tex: &mut GlTexture<TexRed>,
        data: &[u8],
    ) -> RenderedGlyph {
        tex.sub_image(rect, data);
        RenderedGlyph {
            rect: rect,
            bearing: bearing,
            alloc: alloc,
        }
    }

    fn to_tex_color_quad(
        &self,
        pos: Point2D<i32, PixelSize>,
        atlas: &GlTexture<TexRed>,
        color: Color,
    ) -> TexColorQuad {
        let quad_rect = Rect::new(
            point2(
                (pos.x + self.bearing.width) as f32,
                (pos.y - self.bearing.height) as f32,
            ),
            self.rect.size.cast(),
        );
        let tex_rect = atlas.get_inverted_tex_dimension(self.rect.cast());
        TexColorQuad::new(quad_rect, tex_rect, color)
    }
}

/// Handle to glyph renderer
pub(super) struct GlyphRenderer {
    atlas: GlTexture<TexRed>,
    glyph_map: FnvHashMap<GlyphKey, Option<RenderedGlyph>>,
    dpi: Size2D<u32, DPI>,
    allocator: AtlasAllocator,
}

impl GlyphRenderer {
    /// Initialize a new glyph renderer
    pub(super) fn new(gl: &mut Gl, dpi: Size2D<u32, DPI>) -> GlyphRenderer {
        let options = AllocatorOptions {
            snap_size: 1,
            small_size_threshold: 8,
            large_size_threshold: 256,
        };
        GlyphRenderer {
            atlas: gl.new_texture(TexUnit::Texture0, size2(GL_TEX_SIZE, GL_TEX_SIZE)),
            glyph_map: FnvHashMap::default(),
            dpi: dpi,
            allocator: AtlasAllocator::with_options(
                (GL_TEX_SIZE as i32, GL_TEX_SIZE as i32).into(),
                &options,
            ),
        }
    }

    /// Activate renderer
    pub(super) fn activate<'a, 'b>(
        &'a mut self,
        vert_buf: &'b mut ElemArr<TexColorQuad>,
    ) -> ActiveGlyphRenderer<'a, 'b> {
        self.atlas.activate();
        ActiveGlyphRenderer {
            atlas: &mut self.atlas,
            glyph_map: &mut self.glyph_map,
            dpi: self.dpi,
            allocator: &mut self.allocator,
            vert_buf: vert_buf,
        }
    }
}

/// Handle to a glyph renderer with an activated texture
pub(super) struct ActiveGlyphRenderer<'a, 'b> {
    atlas: &'a mut GlTexture<TexRed>,
    glyph_map: &'a mut FnvHashMap<GlyphKey, Option<RenderedGlyph>>,
    dpi: Size2D<u32, DPI>,
    allocator: &'a mut AtlasAllocator,
    vert_buf: &'b mut ElemArr<TexColorQuad>,
}

impl<'a, 'b> ActiveGlyphRenderer<'a, 'b> {
    /// Render a glyph at given coordinate
    pub(super) fn render_glyph(
        &mut self,
        pos: Point2D<i32, PixelSize>, // Baseline
        face: FaceKey,
        gid: u32,
        size: TextSize,
        color: Color,
        style: TextStyle,
        raster: &mut RasterFace,
    ) -> Option<()> {
        let key = GlyphKey {
            gid: gid,
            size: size,
            face: face,
            style: style,
        };
        let optrg = if let Some(optrg) = self.glyph_map.get(&key) {
            optrg
        } else {
            if let Some(rast_glyph) = raster.raster(gid, size, self.dpi) {
                // TODO: Free LRU if allocation fails, and flush text
                // In that case, use bg shader to flush bg quads before flushing text
                // It's better to do that inside TextView. So, indicate the need to flush, using
                // the return value from this function
                let alloc = self
                    .allocator
                    .allocate(rast_glyph.size.cast().to_tuple().into())?;
                let min = alloc.rectangle.min;
                let rg = RenderedGlyph::new(
                    Rect::new(point2(min.x as u32, min.y as u32), rast_glyph.size),
                    size2(rast_glyph.bearing.width, rast_glyph.bearing.height),
                    alloc.id,
                    &mut self.atlas,
                    rast_glyph.buffer,
                );
                self.glyph_map.insert(key, Some(rg));
            } else {
                self.glyph_map.insert(key, None);
            }
            self.glyph_map.get(&key).unwrap()
        };
        if let Some(rg) = optrg {
            let tcq = rg.to_tex_color_quad(pos, &self.atlas, color);
            self.vert_buf.push(tcq);
        }
        Some(())
    }

    /// Flush contents of textured quad buffer
    pub(super) fn flush(&mut self, active_shader: &ActiveShaderProgram) {
        self.vert_buf.flush(active_shader)
    }
}
