// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::collections::HashMap;
use std::ffi::CString;

use euclid::Size2D;

use crate::types::{PixelSize, TextStyle};

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
mod fontconfig;
mod freetype;
pub(in crate::ui) mod harfbuzz;

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
use self::fontconfig as source;
use self::freetype::RasterCore;
pub(in crate::ui) use self::freetype::RasterFace;
use self::harfbuzz::{HbBuffer, HbFont};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::ui) struct FaceKey(u16);

impl FaceKey {
    pub(in crate::ui) fn ival(self) -> u16 {
        self.0
    }
}

pub(in crate::ui) struct Face {
    pub(in crate::ui) raster: RasterFace,
    pub(in crate::ui) shaper: HbFont,
}

impl Face {
    fn new(core: &RasterCore, path: CString, idx: u32) -> Option<Face> {
        let raster = core.new_face(&path, idx)?;
        let shaper = HbFont::new(&path, idx)?;
        Some(Face {
            raster: raster,
            shaper: shaper,
        })
    }
}

struct FaceFamily {
    name: String,
    faces: HashMap<TextStyle, Face>,
}

impl FaceFamily {
    fn empty(name: String) -> FaceFamily {
        FaceFamily {
            name: name,
            faces: HashMap::new(),
        }
    }

    fn set_face(&mut self, style: TextStyle, face: Face) -> Option<&mut Face> {
        self.faces.insert(style, face);
        self.faces.get_mut(&style)
    }
}

pub(in crate::ui) struct FaceGroup {
    family: FaceFamily,
    fallbacks: Vec<FaceKey>,
}

impl FaceGroup {
    fn new(family: String, style: TextStyle, face: Face) -> FaceGroup {
        let mut family = FaceFamily::empty(family);
        family.faces.insert(style, face);
        FaceGroup {
            family: family,
            fallbacks: Vec::new(),
        }
    }
}

pub(in crate::ui) struct FontCore {
    path_face_map: HashMap<(CString, u32), FaceKey>,
    key_face_map: HashMap<FaceKey, FaceGroup>,
    next_key: u16,
    raster_core: RasterCore,
    hb_buffer: HbBuffer,
    source: source::FontSource,
}

impl FontCore {
    pub(in crate::ui) fn new() -> Option<FontCore> {
        let source = source::FontSource::new()?;
        let raster_core = RasterCore::new()?;
        let hb_buffer = HbBuffer::new()?;
        Some(FontCore {
            source: source,
            path_face_map: HashMap::new(),
            key_face_map: HashMap::new(),
            raster_core: raster_core,
            hb_buffer: hb_buffer,
            next_key: 0,
        })
    }

    pub(in crate::ui) fn find(&mut self, family: &str) -> Option<FaceKey> {
        let default_style = TextStyle::default();
        for (key, group) in self.key_face_map.iter() {
            if group.family.name == family {
                return Some(*key);
            }
        }

        let mut pattern = source::Pattern::new()?;
        if !pattern.set_family(family)
            || !pattern.set_slant(default_style.slant)
            || !pattern.set_weight(default_style.weight)
        {
            return None;
        }
        let (family, path, idx) = self.source.find_match(&mut pattern)?;

        if let Some(key) = self.path_face_map.get(&(path.clone(), idx)) {
            Some(*key)
        } else {
            for (key, group) in self.key_face_map.iter() {
                if group.family.name == family {
                    return Some(*key);
                }
            }

            let key = FaceKey(self.next_key);
            let face = Face::new(&self.raster_core, path.clone(), idx)?;
            self.key_face_map
                .insert(key, FaceGroup::new(family, default_style, face));
            self.path_face_map.insert((path, idx), key);
            self.next_key += 1;
            Some(key)
        }
    }

    pub(in crate::ui) fn find_for_char(&mut self, base: FaceKey, c: char) -> Option<FaceKey> {
        let default_style = TextStyle::default();

        let group = self.key_face_map.get(&base)?;
        let face = group.family.faces.get(&default_style)?;
        if face.raster.has_glyph_for_char(c) {
            return Some(base);
        }

        for key in &group.fallbacks {
            let group = self.key_face_map.get(&key)?;
            let face = group.family.faces.get(&default_style)?;
            if face.raster.has_glyph_for_char(c) {
                return Some(*key);
            }
        }

        let mut pattern = source::Pattern::new()?;
        let mut charset = source::Charset::new()?;
        charset.add_char(c);
        if !pattern.set_family(&group.family.name)
            || !pattern.set_slant(default_style.slant)
            || !pattern.set_weight(default_style.weight)
            || !pattern.add_charset(charset)
        {
            return None;
        }
        let (family, path, idx) = self.source.find_match(&mut pattern)?;

        let key = FaceKey(self.next_key);
        let face = Face::new(&self.raster_core, path, idx)?;
        if !face.raster.has_glyph_for_char(c) {
            return None;
        }

        let group = self.key_face_map.get_mut(&base)?;
        group.fallbacks.push(key);
        self.key_face_map
            .insert(key, FaceGroup::new(family, default_style, face));
        self.next_key += 1;
        Some(key)
    }

    pub(in crate::ui) fn get(
        &mut self,
        key: FaceKey,
        style: TextStyle,
    ) -> Option<(&mut HbBuffer, &mut Face)> {
        let hb_buffer = &mut self.hb_buffer;
        let group = self.key_face_map.get_mut(&key)?;
        if group.family.faces.contains_key(&style) {
            return Some((hb_buffer, group.family.faces.get_mut(&style)?));
        }
        let mut pattern = source::Pattern::new()?;
        if !pattern.set_family(&group.family.name)
            || !pattern.set_slant(style.slant)
            || !pattern.set_weight(style.weight)
        {
            return None;
        }
        let (_, path, idx) = self.source.find_match(&mut pattern)?;
        let face = Face::new(&self.raster_core, path, idx)?;
        Some((hb_buffer, group.family.set_face(style, face)?))
    }
}

#[derive(Clone)]
pub(in crate::ui) struct RasterizedGlyph<'a> {
    pub(in crate::ui) size: Size2D<u32, PixelSize>,
    pub(in crate::ui) bearing: Size2D<i32, PixelSize>,
    pub(in crate::ui) buffer: &'a [u8],
}

#[derive(Clone, Debug, Copy)]
pub(in crate::ui) struct ScaledFaceMetrics {
    pub(in crate::ui) ascender: i32,
    pub(in crate::ui) descender: i32,
    pub(in crate::ui) advance_width: i32,
    pub(in crate::ui) underline_pos: i32,
    pub(in crate::ui) underline_thickness: i32,
}
