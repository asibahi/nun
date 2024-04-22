use ab_glyph::{self as ab, Font as _, ScaleFont as _};
use harfbuzz_rs as hb;
use image::{GenericImageView as _, Rgba, RgbaImage};
use imageproc::drawing::Canvas as _;
use nun::LineData;
use resvg::{tiny_skia::Pixmap, usvg};
use std::{ops::Add, path::Path};

const FACTOR: u32 = 4;

const MARGIN: u32 = FACTOR * 100;

const IMG_WIDTH: u32 = FACTOR * 2000;
const LINE_HEIGHT: u32 = FACTOR * 160;

const FONT_SIZE: f32 = FACTOR as f32 * 80.0;

const MSHQ_DEFAULT: f32 = 25.0;
const SPAC_DEFAULT: f32 = 0.0;
macro_rules! my_file {
    () => {
        "qul"
    };
}
static TEXT: &str = include_str!(concat!("../texts/", my_file!(), ".txt"));

const _WHITE: [u8; 4] = [0xFF; 4];
const _BLACK: [u8; 4] = [0x0A, 0x0A, 0x0A, 0xFF];

const _OFF_WHITE: [u8; 4] = [0xFF, 0xFF, 0xF2, 0xFF];
const _OFF_BLACK: [u8; 4] = [0x20, 0x20, 0x20, 0xFF];

const _GOLD_ORNG: [u8; 4] = [0xB4, 0x89, 0x39, 0xFF];
const _NAVY_BLUE: [u8; 4] = [0x13, 0x2A, 0x4A, 0xFF];

const TXT_COLOR: Rgba<u8> = Rgba(_BLACK);
const BKG_COLOR: Rgba<u8> = Rgba(_OFF_WHITE);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let full_text = TEXT.trim();

    let font_data = std::fs::read("fonts/Raqq.ttf")?;

    let mut hb_font = hb::Font::new(hb::Face::from_bytes(&font_data, 0));

    let mut ab_font = ab::FontRef::try_from_slice(&font_data)?;
    let ab_scale = ab_font.pt_to_px_scale(FONT_SIZE).unwrap();

    let ab_scaled_font = ab_font.as_scaled(ab_scale);
    let scale_factor = ab_scaled_font.scale_factor();

    let primary_variation = nun::Variation::new(*b"MSHQ", 0.0, 100.0, MSHQ_DEFAULT, 0);
    let secondary_variation = nun::Variation::new(*b"SPAC", -80.0, 125.0, SPAC_DEFAULT, 1);

    let lines = nun::line_break(
        &mut hb_font,
        full_text,
        ((IMG_WIDTH - 2 * MARGIN) as f32 / scale_factor.horizontal) as u32,
        primary_variation,
        secondary_variation,
    )?;

    let line_count = lines.len();

    let mut canvas =
        RgbaImage::from_pixel(IMG_WIDTH, line_count as u32 * LINE_HEIGHT + 2 * MARGIN, BKG_COLOR);

    for (idx, line) in lines.into_iter().enumerate() {
        write_in_image(full_text, &mut canvas, idx, &mut ab_font, &mut hb_font, line);
    }

    draw_signature(&mut canvas);

    let path = format!("images/{}_{:.0}.png", my_file!(), MSHQ_DEFAULT);
    let save_file = Path::new(&path);

    canvas.save(save_file)?;

    Ok(())
}

fn draw_signature(canvas: &mut RgbaImage) {
    // hacky function because I don't understand SVGs.
    let (_, height) = canvas.dimensions();

    static STAMP_SVG: &str = include_str!("../personal_stamp.svg");
    let tree =
        usvg::Tree::from_str(STAMP_SVG, &usvg::Options::default(), &usvg::fontdb::Database::new())
            .unwrap();

    let size = tree.size().to_int_size();
    let mut pixmap = Pixmap::new(size.width(), size.height()).unwrap();

    resvg::render(
        &tree,
        usvg::Transform::from_scale(FACTOR as f32 / 4.0, FACTOR as f32 / 4.0),
        &mut pixmap.as_mut(),
    );
    let top = RgbaImage::from_raw(size.width(), size.height(), pixmap.data().to_vec()).unwrap();

    image::imageops::overlay(canvas, &top, MARGIN as i64 / 4, (height - MARGIN) as i64);
}

fn write_in_image(
    full_text: &str,
    canvas: &mut RgbaImage,
    line_number: usize,
    ab_font: &mut (impl ab::Font + ab::VariableFont),
    hb_font: &mut hb::Owned<hb::Font<'_>>,
    LineData { start_bp, end_bp, variations }: LineData<2>,
) {
    nun::Variation::set_variations(variations, ab_font, hb_font);

    let hb_buffer = hb::UnicodeBuffer::new().add_str_item(full_text, &full_text[start_bp..end_bp]);
    let hb_output = hb::shape(hb_font, hb_buffer, &[]);

    let ab_scale = ab_font.pt_to_px_scale(FONT_SIZE).unwrap();
    let ab_scaled_font = ab_font.as_scaled(ab_scale);

    let scale_factor = ab_scaled_font.scale_factor();
    let ascent = ab_scaled_font.ascent();

    // to align everything to the right. works around the weird shaping bug
    let line_width = hb_output
        .get_glyph_positions()
        .iter()
        .map(|p| p.x_advance as f32 * scale_factor.horizontal)
        .reduce(Add::add)
        .unwrap_or_default() as u32;
    // except basmalas to the center.
    let line_width = line_width + (IMG_WIDTH - 2 * MARGIN).saturating_sub(line_width) / 2;

    let mut caret = 0;
    let mut colored_glyphs = vec![];

    for (position, info) in hb_output.get_glyph_positions().iter().zip(hb_output.get_glyph_infos())
    {
        let gl = ab::GlyphId(info.codepoint as u16).with_scale_and_position(
            ab_scale,
            ab::point(
                (caret + position.x_offset) as f32 * scale_factor.horizontal,
                ascent - (position.y_offset as f32 * scale_factor.vertical),
            ),
        );

        caret += position.x_advance;

        let Some(outlined_glyph) = ab_font.outline_glyph(gl) else {
            // gl is whitespace
            continue;
        };

        let bb = outlined_glyph.px_bounds();
        let bbx = bb.min.x as u32 + (IMG_WIDTH - MARGIN - line_width);
        let bby = bb.min.y as u32 + MARGIN + line_number as u32 * LINE_HEIGHT;

        if let Some(colored_glyph) = ab_font
            .glyph_svg_image(ab::GlyphId(info.codepoint as u16))
            .and_then(|svg| svg_data_to_glyph(svg.data, bb, info.codepoint))
        {
            colored_glyphs.push((bbx, bby, colored_glyph));
        } else {
            outlined_glyph.draw(|px, py, pv| {
                let px = px + bbx;
                let py = py + bby;
                let pv = pv.clamp(0.0, 1.0);

                if canvas.in_bounds(px, py) {
                    let pixel = canvas.get_pixel(px, py).to_owned();
                    let weighted_color = imageproc::pixelops::interpolate(TXT_COLOR, pixel, pv);
                    canvas.draw_pixel(px, py, weighted_color);
                }
            });
        }
    }

    for (bbx, bby, colored_glyph) in colored_glyphs.into_iter().rev() {
        image::imageops::overlay(canvas, &colored_glyph, bbx.into(), bby.into());
    }
}

fn svg_data_to_glyph(data: &[u8], bb: ab::Rect, codepoint: u32) -> Option<RgbaImage> {
    let tree =
        usvg::Tree::from_data(data, &usvg::Options::default(), &usvg::fontdb::Database::new())
            .ok()?;
    let node = tree.node_by_id(&format!("glyph{codepoint}"))?;
    let size = node.abs_layer_bounding_box()?;
    let transform =
        usvg::Transform::from_scale(bb.width() / size.width(), bb.height() / size.height());

    let size = size.to_int_rect();
    let mut pixmap = Pixmap::new(size.width(), size.height())?;

    resvg::render_node(node, transform, &mut pixmap.as_mut());
    RgbaImage::from_raw(size.width(), size.height(), pixmap.data().to_vec())
}
