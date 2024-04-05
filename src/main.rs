// #![allow(unused)]

use ab::VariableFont;
use ab_glyph::{self as ab, Font as _, ScaleFont as _};
use harfbuzz_rs as hb;
use image::{GenericImageView, RgbaImage};
use imageproc::drawing::Canvas as _;
use noor::LineData;
use std::path::Path;

const FACTOR: u32 = 1;

const MARGIN: u32 = FACTOR * 100;

const IMG_WIDTH: u32 = FACTOR * 2000;
const LINE_HEIGHT: u32 = FACTOR * 150;

const FONT_SIZE: f32 = FACTOR as f32 * 80.0;

const BASE_STRETCH: f32 = 50.0;
macro_rules! my_file {
    () => {
        "kursi"
    };
}
static TEXT: &str = include_str!(concat!("../lines/", my_file!(), ".txt"));

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let font_data = std::fs::read("fonts/Raqq.ttf")?;

    let mut hb_font = hb::Font::new(hb::Face::from_bytes(&font_data, 0));

    let mut ab_font = ab::FontRef::try_from_slice(&font_data)?;
    let ab_scale = ab_font.pt_to_px_scale(FONT_SIZE).unwrap();

    let ab_scaled_font = ab_font.as_scaled(ab_scale);
    let scale_factor = ab_scaled_font.scale_factor();

    let lines = noor::line_break(
        &mut hb_font,
        TEXT,
        IMG_WIDTH - 2 * MARGIN,
        scale_factor.horizontal,
        BASE_STRETCH,
    )?;

    let mut canvas: image::RgbaImage = image::ImageBuffer::from_pixel(
        IMG_WIDTH,
        lines.len() as u32 * LINE_HEIGHT + 2 * MARGIN,
        image::Rgba([10, 10, 10, 255]),
    );

    for (idx, line) in lines.into_iter().enumerate() {
        write_in_image(&mut canvas, idx, &mut ab_font, &mut hb_font, line);
    }

    let path = format!("lines/{}_{:.0}.png", my_file!(), BASE_STRETCH);
    let save_file = Path::new(&path);

    canvas.save(save_file)?;

    Ok(())
}

fn write_in_image(
    canvas: &mut RgbaImage,
    line: usize,
    ab_font: &mut ab::FontRef<'_>,
    hb_font: &mut hb::Owned<hb::Font<'_>>,
    LineData {
        start_bp,
        end_bp,
        mshq_val,
        spac_val,
    }: LineData,
) {
    hb_font.set_variations(&[
        hb::Variation::new(noor::MSHQ, mshq_val),
        hb::Variation::new(noor::SPAC, spac_val),
    ]);

    let buffer = hb::UnicodeBuffer::new().add_str(TEXT[start_bp..end_bp].trim());
    let output = hb::shape(&hb_font, buffer, &[]);

    ab_font.set_variation(noor::MSHQ, mshq_val);
    ab_font.set_variation(noor::SPAC, spac_val);

    let ab_scale = ab_font.pt_to_px_scale(FONT_SIZE).unwrap();

    let ab_scaled_font = ab_font.as_scaled(ab_scale);
    let scale_factor = ab_scaled_font.scale_factor();

    let ascent = ab_scaled_font.ascent();

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
            let py = py + bb.min.y as u32 + MARGIN + line as u32 * LINE_HEIGHT;

            if canvas.in_bounds(px, py) {
                let pixel = canvas.get_pixel(px, py).to_owned();
                let color = image::Rgba([255; 4]);
                let weighted_color = imageproc::pixelops::interpolate(color, pixel, pv);
                canvas.draw_pixel(px, py, weighted_color);
            }
        });
    }
}
