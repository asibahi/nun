use std::path::Path;

use ab_glyph::{self as ab, Font as _, ScaleFont as _};
use harfbuzz_rs as hb;
use imageproc::drawing::Canvas as _;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let font_path = "fonts/Raqq.ttf";

    let buffer = hb::UnicodeBuffer::new().add_str("الله نور السموت والارض");

    let font_data = std::fs::read(font_path)?;

    let index = 0; //< face index in the font file
    let mut hb_font = hb::Font::new(hb::Face::from_bytes(&font_data, index));

    hb_font.set_variations(&[hb::Variation::new(b"wdth", 100.0)]); // variable font

    let output = hb::shape(&hb_font, buffer, &[]);

    let positions = output.get_glyph_positions();
    let infos = output.get_glyph_infos();

    let width: i32 = positions.iter().map(|gp| gp.x_advance).sum();

    let ab_font = ab::FontRef::try_from_slice(&font_data)?;

    let mut caret = 0;

    let ab_scale = ab_font.pt_to_px_scale(60.0).unwrap();

    let ab_scaled_font = ab_font.as_scaled(ab_scale);
    let scale_factor = ab_scaled_font.scale_factor();

    let ascent = ab_scaled_font.ascent();

    let width = (width as f32 * scale_factor.horizontal) as u32 + 30;
    dbg!(width);
    let height = 300;

    let mut canvas: image::RgbaImage = image::ImageBuffer::new(width, height);
    imageproc::drawing::draw_filled_rect_mut(
        &mut canvas,
        imageproc::rect::Rect::at(0, 0).of_size(width, height),
        image::Rgba([255, 255, 255, 255]),
    );

    for (position, info) in positions.iter().zip(infos) {
        let gid = info.codepoint;
        let cluster = info.cluster;
        let x_advance = position.x_advance;
        let x_offset = position.x_offset;
        let y_offset = position.y_offset;

        println!(
            "gid{:?}={:?}@{:?},{:?}+{:?}",
            gid, cluster, x_advance, x_offset, y_offset
        );

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

        // USELESS BLUE BOUNDING BOX
        for x in bb.min.x as u32..=bb.max.x as u32 {
            canvas.draw_pixel(x + 15, bb.min.y as u32 + 15, image::Rgba([0, 0, 255, 255]));
            canvas.draw_pixel(x + 15, bb.max.y as u32 + 15, image::Rgba([0, 0, 255, 255]));
        }
        for y in bb.min.y as u32..=bb.max.y as u32 {
            canvas.draw_pixel(bb.min.x as u32 + 15, y + 15, image::Rgba([0, 0, 255, 255]));
            canvas.draw_pixel(bb.max.x as u32 + 15, y + 15, image::Rgba([0, 0, 255, 255]));
        }

        outlined_glyph.draw(|px, py, pv| {
            let px = px + bb.min.x as u32 + 15;
            let py = py + bb.min.y as u32 + 15;

            let pixel = canvas.get_pixel(px, py).to_owned();
            let color = image::Rgba([0, 0, 0, 255]);
            let weighted_color = imageproc::pixelops::interpolate(color, pixel, pv);
            canvas.draw_pixel(px, py, weighted_color);
        });
    }

    // USELESS BLUE LINE
    for i in 0..15 {
        canvas.draw_pixel(i, i, image::Rgba([0, 0, 255, 255]));
        canvas.draw_pixel(i + 1, i, image::Rgba([0, 0, 255, 255]));
        canvas.draw_pixel(i, i + 1, image::Rgba([0, 0, 255, 255]));
    }

    let save_file = Path::new("fff.png");

    canvas.save(save_file)?;

    Ok(())
}
