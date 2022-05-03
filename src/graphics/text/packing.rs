use std::{hash::Hash, collections::HashMap, cmp};

use nalgebra::Vector2;

pub type Coord = u32;

pub trait GlyphSize<ID> {
    fn id(&self) -> ID;
    fn width(&self) -> Coord;
    fn height(&self) -> Coord;
}

struct GlyphInfo<ID: Eq + Hash> {
    id: ID,
    width: Coord,
    height: Coord
}

impl<ID: Eq + Hash + Clone> GlyphInfo<ID> {
    fn copy<T: GlyphSize<ID>>(glyph: &T) -> GlyphInfo<ID> {
        GlyphInfo {
            id: glyph.id(),
            width: glyph.width(),
            height: glyph.height()
        }
    }
}

impl<ID> GlyphSize<ID> for GlyphInfo<ID> where ID: Eq + Hash + Clone {
    fn id(&self) -> ID {
        self.id.clone()
    }
    fn width(&self) -> Coord {
        self.width
    }
    fn height(&self) -> Coord {
        self.height
    }
}

pub struct GlyphPacking<ID> where ID: Eq + Hash + Clone {
    width: Coord,
    height: Coord,
    pos_map: HashMap<ID, Vector2<Coord>> 
}

impl<ID> GlyphPacking<ID> where ID: Eq + Hash + Clone {
    pub fn width(&self) -> Coord {
        self.width
    }
    pub fn height(&self) -> Coord {
        self.height
    }
    pub fn get_glyph_pos(&self, char_index: ID) -> Option<Vector2<Coord>> {
        match self.pos_map.get(&char_index) {
            Some(pos) => Some(*pos),
            None => None
        }
    }
}

pub fn do_font_packing<ID, T>(glyphs: &Vec<T>) -> Option<GlyphPacking<ID>>
        where ID: Eq + Hash + Clone, T: GlyphSize<ID> {
    let min = 6;
    let max = 14;
    let mut glyphs_copy: Vec<GlyphInfo<ID>> = glyphs.iter().map(|glyph: &T| GlyphInfo::copy(glyph)).collect();
    // sort descending height
    glyphs_copy.sort_by(|a, b| b.height().cmp(&a.height()));
    // try to pack the glyphs
    recursive(&glyphs_copy, min, max)
}

fn recursive<ID, T>(glyphs: &Vec<T>, min: Coord, max: Coord) -> Option<GlyphPacking<ID>>
        where ID: Eq + Hash + Clone, T: GlyphSize<ID> {
    assert!(min < 32 && max < 32);
    if max == min {
        // only one option
        let size = (2 as Coord).pow(min);
        get_packing(glyphs, size, size)
    } else {
        let log_size = (min + max + 1) / 2; // ceiling division
        let size = (2 as Coord).pow(log_size);
        match get_packing(glyphs, size, size) {
            Some(packing) =>
                // valid try smaller
                match recursive(glyphs, min, cmp::max(log_size - 1, min)) {
                    Some(smaller) => Some(smaller),
                    None => Some(packing)
                },
            None =>
                // invalid go bigger
                if log_size == max {
                    None // already biggest - this should affect the case
                         // where the range is 2 sizes. without this,
                         // if both values are fails, the bigger will calculate twice
                } else {
                    recursive(glyphs, cmp::min(log_size + 1, max), max)
                }
        }
    }
}

// does the simplest possible packing algorithm
// tries to fill up a row, moves to the next row if it's full
// works best if glyphs is sorted somehow
fn get_packing<ID, T>(glyphs: &Vec<T>, width: Coord, height: Coord) -> Option<GlyphPacking<ID>>
        where ID: Eq + Hash + Clone, T: GlyphSize<ID> {
    let mut map: HashMap<ID, Vector2<Coord>> = HashMap::new();
    // fill up a row then go next row
    let mut row_width = 0;
    let mut row_start_y = 0;
    let mut row_end_y = 0;
    let mut overflow = false;
    for glyph in glyphs {
        if glyph.width() > width {
            overflow = true;
            break;
        }
        if row_width + glyph.width() > width {
            // move down a row
            row_width = 0;
            row_start_y = row_end_y;
        }
        if cmp::max(row_end_y, row_start_y + glyph.height()) <= height {
            // place glyph
            map.insert(glyph.id(), Vector2::new(row_width, row_start_y));
            row_width += glyph.width();
            row_end_y = cmp::max(row_end_y, row_start_y + glyph.height());
        } else {
            overflow = true;
            break;
        }
    }
    if overflow {
        None
    } else {
        Some(GlyphPacking {
            width: width,
            height: height,
            pos_map: map
        })
    }
}