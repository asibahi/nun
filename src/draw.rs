use crate::{
    logic::{line_break, Variation, VariationKind},
    shaper::Shaper,
};
use ab_glyph::{self as ab, Font as _, ScaleFont as _};
use image::{GenericImageView as _, Rgba, RgbaImage};
use imageproc::drawing::Canvas as _;
use resvg::{tiny_skia::Pixmap, usvg};
use std::path::Path;

#[derive(Clone, Copy)]
pub struct ImageConfig {
    pub margin: u32,
    pub img_width: u32,
    pub font_size: f32,
    pub txt_color: [u8; 4],
    pub bkg_color: [u8; 4],
}

pub fn run<const N: usize>(
    text_path: impl AsRef<Path>,
    font_path: impl AsRef<Path>,
    variations: [Variation; N],
    config @ ImageConfig { margin, img_width, font_size, txt_color: _, bkg_color }: ImageConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let full_text = std::fs::read_to_string(text_path.as_ref())?;
    let font_data = std::fs::read(font_path)?;

    // let mut shaper = HarfBuzz::new(&font_data);
    let mut shaper = crate::shaper::RustBuzz::new(&font_data);

    let mut ab_font = ab::FontRef::try_from_slice(&font_data)?;
    let ab_scale = ab_font.pt_to_px_scale(font_size).unwrap();

    let ab_scaled_font = ab_font.as_scaled(ab_scale);
    let scale_factor = ab_scaled_font.scale_factor();
    let ascent = ab_scaled_font.ascent();

    let lines = line_break(
        &mut shaper,
        &full_text,
        ((img_width - 2 * margin) as f32 / scale_factor.horizontal) as u32,
        variations,
    )?;

    let line_height = (ab_scaled_font.height() * 1.25) as u32;

    let mut canvas = RgbaImage::from_pixel(
        img_width,
        lines.len() as u32 * line_height + 2 * margin,
        Rgba(bkg_color),
    );

    for (idx, line) in lines.into_iter().enumerate() {
        let text_slice = {
            let t = full_text[line.start_bp..line.end_bp].trim();
            let c = kashida::find_kashidas(t, kashida::Script::Arabic);
            kashida::place_kashidas(t, &c, line.kashida_count)
        };

        write_in_image(
            &text_slice,
            &mut canvas,
            idx,
            &mut ab_font,
            &mut shaper,
            line.variations,
            config,
            ScaledFontData { line_height, scale_factor, ascent, ab_scale },
        );
    }

    _ = draw_signature(&mut canvas, margin);

    canvas.save(&text_path.as_ref().with_extension("png"))?;

    Ok(())
}

fn draw_signature(canvas: &mut RgbaImage, margin: u32) -> Result<(), Box<dyn std::error::Error>> {
    // hacky function because I don't understand SVGs.  buggy af.

    // No idea how to position things correctly so I just have to remember changing this when I change it in `main.rs`
    const FACTOR: f32 = 4.0;

    let (_, height) = canvas.dimensions();

    let Ok(stamp_svg) = std::fs::read_to_string("personal_stamp.svg") else { return Ok(()) };
    let tree = usvg::Tree::from_str(&stamp_svg, &usvg::Options::default())?;

    let size = tree.size().to_int_size();
    let mut pixmap = Pixmap::new(size.width(), size.height()).ok_or("")?;

    resvg::render(
        &tree,
        usvg::Transform::from_scale(FACTOR / 4.0, FACTOR / 4.0),
        &mut pixmap.as_mut(),
    );
    let top = RgbaImage::from_raw(size.width(), size.height(), pixmap.data().to_vec()).ok_or("")?;

    image::imageops::overlay(canvas, &top, margin as i64 / 4, (height - margin) as i64);

    Ok(())
}

struct ScaledFontData {
    line_height: u32,
    scale_factor: ab::PxScaleFactor,
    ascent: f32,
    ab_scale: ab::PxScale,
}

#[allow(clippy::too_many_arguments)]
fn write_in_image<'a, const N: usize>(
    text_slice: &str,
    canvas: &mut RgbaImage,
    line_number: usize,
    ab_font: &mut (impl ab::Font + ab::VariableFont),
    shaper: &mut impl Shaper<'a>,
    variations: [Variation; N],
    ImageConfig { margin, img_width: _, font_size: _, txt_color, bkg_color: _ }: ImageConfig,
    ScaledFontData { line_height, scale_factor, ascent, ab_scale }: ScaledFontData,
) {
    variations
        .iter()
        .filter_map(|v| match v.kind {
            VariationKind::Axis(tag) => Some((tag, v.current_value)),
            VariationKind::Spacing => None,
        })
        .for_each(|(tag, value)| {
            ab_font.set_variation(&tag, value);
        });

    let shaped_text = shaper.shape_text(text_slice, &variations);

    let centered_line_offset = (canvas.width() - 2 * margin).saturating_sub(
        shaped_text.iter().map(|g| g.x_advance as f32 * scale_factor.horizontal).sum::<f32>()
            as u32,
    ) / 2;

    let mut caret = 0;
    let mut colored_glyphs = vec![];

    for glyph in shaped_text {
        let gl = ab::GlyphId(glyph.codepoint as u16).with_scale_and_position(
            ab_scale,
            ab::point(
                (caret + glyph.x_offset) as f32 * scale_factor.horizontal,
                ascent - (glyph.y_offset as f32 * scale_factor.vertical),
            ),
        );

        caret += glyph.x_advance;
        let Some(outlined_glyph) = ab_font.outline_glyph(gl) else {
            // gl is whitespace?
            continue;
        };

        let bb = outlined_glyph.px_bounds();
        let bbx = (bb.min.x as i32).saturating_add_unsigned(margin + centered_line_offset);
        let bby =
            (bb.min.y as i32).saturating_add_unsigned(margin + line_number as u32 * line_height);
        if let Some(colored_glyph) = ab_font
            .glyph_svg_image(ab::GlyphId(glyph.codepoint as u16))
            .and_then(|svg| svg_data_to_glyph(svg.data, bb, glyph.codepoint))
        {
            colored_glyphs.push((bbx, bby, colored_glyph));
        } else {
            outlined_glyph.draw(|px, py, pv| {
                let px = px.saturating_add_signed(bbx);
                let py = py.saturating_add_signed(bby);
                let pv = pv.clamp(0.0, 1.0);

                if canvas.in_bounds(px, py) {
                    let pixel = canvas.get_pixel(px, py).to_owned();
                    let weighted_color =
                        imageproc::pixelops::interpolate(Rgba(txt_color), pixel, pv);
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
    let tree = usvg::Tree::from_data(data, &usvg::Options::default()).ok()?;
    let node = tree.node_by_id(&format!("glyph{codepoint}"))?;
    let size = node.abs_layer_bounding_box()?;
    let transform =
        usvg::Transform::from_scale(bb.width() / size.width(), bb.height() / size.height());

    let size = size.to_int_rect();
    let mut pixmap = Pixmap::new(size.width(), size.height())?;

    resvg::render_node(node, transform, &mut pixmap.as_mut());
    RgbaImage::from_raw(size.width(), size.height(), pixmap.data().to_vec())
}
