use std::path::Path;

use ab::VariableFont;
use ab_glyph::{self as ab, Font as _, ScaleFont as _};
use harfbuzz_rs as hb;
use imageproc::drawing::Canvas as _;

const MSHQ: &[u8; 4] = b"MSHQ";
// const MSHQ_MIN: f32 = 0.0;
const MSHQ_AVG: f32 = 50.0;
// const MSHQ_MAX: f32 = 100.0;

const SPAC: &[u8; 4] = b"SPAC";
const SPAC_VAL: f32 = 0.0;

const MARGIN: u32 = 15;

const IMG_WIDTH: u32 = 2000;
const IMG_HEIGHT: u32 = 2000;

static TEXT: &str = include_str!("../noortest.txt");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let buffer = hb::UnicodeBuffer::new().add_str(TEXT.trim());

    let font_data = std::fs::read("fonts/Raqq.ttf")?;

    let hb_font = {
        let mut font = hb::Font::new(hb::Face::from_bytes(&font_data, 0));

        font.set_variations(&[
            hb::Variation::new(MSHQ, MSHQ_AVG),
            hb::Variation::new(SPAC, SPAC_VAL),
        ]);

        font
    };

    let output = hb::shape(&hb_font, buffer, &[]);

    let positions = output.get_glyph_positions();
    let infos = output.get_glyph_infos();

    let mut ab_font = ab::FontRef::try_from_slice(&font_data)?;
    ab_font.set_variation(SPAC, SPAC_VAL);
    ab_font.set_variation(MSHQ, MSHQ_AVG);

    let canvas = fun_name(ab_font, positions, infos);

    let save_file = Path::new("fff.png");

    canvas.save(save_file)?;

    Ok(())
}

fn fun_name<'a>(
    ab_font: ab::FontRef<'a>,
    positions: &[hb::GlyphPosition],
    infos: &[hb::GlyphInfo],
) -> image::RgbaImage {
    let ab_scale = ab_font.pt_to_px_scale(60.0).unwrap();

    let ab_scaled_font = ab_font.as_scaled(ab_scale);
    let scale_factor = ab_scaled_font.scale_factor();

    let ascent = ab_scaled_font.ascent();

    let mut canvas: image::RgbaImage =
        image::ImageBuffer::from_pixel(IMG_WIDTH, IMG_HEIGHT, image::Rgba([255; 4]));

    let mut caret = 0;

    for (position, info) in positions.iter().zip(infos) {
        let gid = info.codepoint;
        let x_advance = position.x_advance;
        let x_offset = position.x_offset;
        let y_offset = position.y_offset;

        let horizontal = (caret + x_offset) as f32 * scale_factor.horizontal;
        let vertical = ascent - (y_offset as f32 * scale_factor.vertical);

        let gl = ab_glyph::GlyphId(gid as u16)
            .with_scale_and_position(ab_scale, ab_glyph::point(horizontal, vertical));

        caret += x_advance;

        let Some(outlined_glyph) = ab_font.outline_glyph(gl) else {
            // gl is whitespace
            continue;
        };

        let bb = outlined_glyph.px_bounds();

        outlined_glyph.draw(|px, py, pv| {
            let px = px + bb.min.x as u32 + MARGIN;
            let py = py + bb.min.y as u32 + MARGIN;

            let pixel = canvas.get_pixel(px, py).to_owned();
            let color = image::Rgba([0, 0, 0, 255]);
            let weighted_color = imageproc::pixelops::interpolate(color, pixel, pv);
            canvas.draw_pixel(px, py, weighted_color);
        });
    }
    canvas
}
