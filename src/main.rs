#![allow(unused)]

use ab::VariableFont;
use ab_glyph::{self as ab, Font as _, ScaleFont as _};
use harfbuzz_rs as hb;
use image::GenericImageView;
use imageproc::drawing::Canvas as _;
use noor::LineData;
use std::path::Path;

const MARGIN: u32 = 100;

const IMG_WIDTH: u32 = 2000;
const IMG_HEIGHT: u32 = 2000;

static TEXT: &str = include_str!("../noor.txt");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let font_data = std::fs::read("fonts/Raqq.ttf")?;

    let mut hb_font = hb::Font::new(hb::Face::from_bytes(&font_data, 0));

    let mut ab_font = ab::FontRef::try_from_slice(&font_data)?;
    let ab_scale = ab_font.pt_to_px_scale(80.0).unwrap();

    let ab_scaled_font = ab_font.as_scaled(ab_scale);
    let scale_factor = ab_scaled_font.scale_factor();

    let line_data = noor::find_optimal_line(
        &mut hb_font,
        TEXT,
        0,
        42,
        IMG_WIDTH - 2 * MARGIN,
        scale_factor.horizontal,
    )?;

    let canvas = line_data_to_image(ab_font, hb_font, line_data);

    let save_file = Path::new("fff.png");

    canvas.save(save_file)?;

    Ok(())
}

fn line_data_to_image<'a>(
    mut ab_font: ab::FontRef<'a>,
    hb_font: hb::Owned<hb::Font<'_>>,
    LineData {
        start_bp,
        end_bp,
        variation_value,
    }: LineData,
) -> image::RgbaImage {
    let buffer = hb::UnicodeBuffer::new().add_str(TEXT[start_bp..end_bp].trim());
    let output = hb::shape(&hb_font, buffer, &[]);

    ab_font.set_variation(noor::SPAC, noor::SPAC_VAL);
    ab_font.set_variation(noor::MSHQ, variation_value);

    let ab_scale = ab_font.pt_to_px_scale(80.0).unwrap();

    let ab_scaled_font = ab_font.as_scaled(ab_scale);
    let scale_factor = ab_scaled_font.scale_factor();

    let ascent = ab_scaled_font.ascent();

    let mut canvas: image::RgbaImage =
        image::ImageBuffer::from_pixel(IMG_WIDTH, IMG_HEIGHT, image::Rgba([255; 4]));

    let mut caret = 0;

    for (position, info) in output
        .get_glyph_positions()
        .iter()
        .zip(output.get_glyph_infos())
    {
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

            if canvas.in_bounds(px, py) {
                let pixel = canvas.get_pixel(px, py).to_owned();
                let color = image::Rgba([0, 0, 0, 255]);
                let weighted_color = imageproc::pixelops::interpolate(color, pixel, pv);
                canvas.draw_pixel(px, py, weighted_color);
            }
        });
    }
    canvas
}
