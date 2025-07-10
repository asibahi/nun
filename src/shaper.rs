#![allow(dead_code)]
#![allow(unused)]

use rustybuzz::{ttf_parser::Tag, Feature};

use crate::{logic::VariationKind, Variation};

pub(crate) struct GlyphData {
    pub codepoint: u32,
    pub cluster: u32,
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
}

pub trait Shaper<'f> {
    fn shape_text(
        &mut self,
        input: &str,
        variations: &[Variation],
    ) -> Vec<GlyphData>;
}

pub(crate) struct RustBuzz<'f>(rustybuzz::Face<'f>, Vec<Feature>);
impl<'f> RustBuzz<'f> {
    pub fn new(
        font_data: &'f [u8],
        features: &[[u8; 4]],
    ) -> Self {
        let features = features
            .iter()
            .map(|f| Feature::new(Tag::from_bytes(f), 0, ..))
            .collect();
        Self(rustybuzz::Face::from_slice(font_data, 0).unwrap(), features)
    }
}
impl<'f> Shaper<'f> for RustBuzz<'f> {
    fn shape_text(
        &mut self,
        input: &str,
        variations: &[Variation],
    ) -> Vec<GlyphData> {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(input);

        self.0.set_variations(
            &variations
                .iter()
                .filter_map(|v| match v.kind {
                    VariationKind::Axis(tag) => Some(rustybuzz::Variation {
                        tag: Tag::from_bytes(&tag),
                        value: v.current_value,
                    }),
                    VariationKind::Spacing => None,
                })
                .collect::<Vec<_>>(),
        );

        let output = rustybuzz::shape(&self.0, &self.1, buffer);

        let space = self
            .0
            .glyph_index(' ')
            .expect("Font does not hace a space character.");
        let adjust_space = |space_width| match variations
            .iter()
            .find(|v| matches!(v.kind, VariationKind::Spacing))
        {
            Some(v) => (space_width as f32 * v.current_value) as i32,
            None => space_width,
        };

        output
            .glyph_infos()
            .iter()
            .zip(output.glyph_positions())
            .map(|(i, p)| GlyphData {
                codepoint: i.glyph_id,
                cluster: i.cluster,
                x_advance: if i.glyph_id == space.0 as u32 {
                    adjust_space(p.x_advance)
                } else {
                    p.x_advance
                },
                y_advance: p.y_advance,
                x_offset: p.x_offset,
                y_offset: p.y_offset,
            })
            .collect()
    }
}

// pub(crate) struct HarfBuzz<'f>(harfbuzz_rs::Owned<harfbuzz_rs::Font<'f>>);
// impl<'f> HarfBuzz<'f> {
//     pub fn new(font_data: &'f [u8]) -> Self {
//         Self(harfbuzz_rs::Font::new(harfbuzz_rs::Face::from_bytes(font_data, 0)))
//     }
// }
// impl<'f> Shaper<'f> for HarfBuzz<'f> {
//     fn shape_text(&mut self, input: &str, variations: &[Variation]) -> Vec<GlyphData> {
//         let buffer = harfbuzz_rs::UnicodeBuffer::new().add_str(input);
//         self.0.set_variations(
//             &variations
//                 .iter()
//                 .filter_map(|v| match v.kind {
//                     VariationKind::Axis(tag) => {
//                         Some(harfbuzz_rs::Variation::new(&tag, v.current_value))
//                     }
//                     VariationKind::Spacing => None,
//                 })
//                 .collect::<Vec<_>>(),
//         );

//         let output = harfbuzz_rs::shape(&self.0, buffer, &[]);

//         let space = self.0.get_nominal_glyph(' ').expect("Font does not hace a space character.");
//         let space_width = self.0.get_glyph_h_advance(space);
//         let space_width = match variations.iter().find(|v| matches!(v.kind, VariationKind::Spacing))
//         {
//             Some(v) => (space_width as f32 * v.current_value) as i32,
//             None => space_width,
//         };

//         output
//             .get_glyph_infos()
//             .iter()
//             .zip(output.get_glyph_positions())
//             .map(|(i, p)| GlyphData {
//                 codepoint: i.codepoint,
//                 cluster: i.cluster,
//                 x_advance: if i.codepoint == space { space_width } else { p.x_advance },
//                 y_advance: p.y_advance,
//                 x_offset: p.x_offset,
//                 y_offset: p.y_offset,
//             })
//             .collect()
//     }
// }
