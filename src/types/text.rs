// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::default::Default;
use std::hash::{Hash, Hasher};

use euclid::{size2, Size2D};

use super::{PixelSize, DPI};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextWeight {
    Medium,
    Light,
    Bold,
}

impl TextWeight {
    pub(crate) fn from_str(s: &str) -> Option<TextWeight> {
        match s {
            "medium" => Some(TextWeight::Medium),
            "light" => Some(TextWeight::Light),
            "bold" => Some(TextWeight::Bold),
            _ => None,
        }
    }
}

impl Default for TextWeight {
    fn default() -> TextWeight {
        TextWeight::Medium
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextSlant {
    Roman,
    Italic,
    Oblique,
}

impl TextSlant {
    pub(crate) fn from_str(s: &str) -> Option<TextSlant> {
        match s {
            "roman" => Some(TextSlant::Roman),
            "italic" => Some(TextSlant::Italic),
            "oblique" => Some(TextSlant::Oblique),
            _ => None,
        }
    }
}

impl Default for TextSlant {
    fn default() -> TextSlant {
        TextSlant::Roman
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TextStyle {
    pub(crate) weight: TextWeight,
    pub(crate) slant: TextSlant,
}

impl TextStyle {
    pub(crate) fn new(weight: TextWeight, slant: TextSlant) -> TextStyle {
        TextStyle {
            weight: weight,
            slant: slant,
        }
    }

    pub(crate) fn ival(&self) -> u8 {
        let ret = match self.weight {
            TextWeight::Medium => 0,
            TextWeight::Light => 3,
            TextWeight::Bold => 6,
        };
        match self.slant {
            TextSlant::Roman => ret + 0,
            TextSlant::Italic => ret + 1,
            TextSlant::Oblique => ret + 2,
        }
    }
}

impl Hash for TextStyle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ival().hash(state)
    }
}

// Text size in points
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct TextSize(u16);

impl TextSize {
    pub(crate) fn ival(self) -> u16 {
        self.0
    }

    pub(crate) fn from_f32(f: f32) -> TextSize {
        TextSize((f * TextSize::scale()) as u16)
    }

    pub(crate) fn to_f32(self) -> f32 {
        (self.0 as f32) / TextSize::scale()
    }

    pub(crate) fn to_64th_point(self) -> i64 {
        (self.0 as i64) << (6 - TextSize::shift())
    }

    pub(crate) fn to_pixel_size(self, dpi: Size2D<u32, DPI>) -> Size2D<f32, PixelSize> {
        let val = self.to_f32() / 72.0;
        size2(val * dpi.width as f32, val * dpi.height as f32)
    }

    pub(crate) fn scale() -> f32 {
        4.0
    }

    pub(crate) fn shift() -> usize {
        2
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextPitch {
    Fixed,
    Variable,
}
