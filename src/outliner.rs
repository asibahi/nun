#![allow(unused)]

use ab_glyph::{point, OutlineCurve, Point};
use image::GenericImageView as _;
use owned_ttf_parser as ttfp;

#[derive(Debug, Default)]
pub(crate) struct OutlineCurveBuilder {
    last: Point,
    last_move: Option<Point>,
    outline: Vec<OutlineCurve>,
}

impl OutlineCurveBuilder {
    #[inline]
    pub(crate) fn take_outline(mut self) -> Vec<OutlineCurve> {
        // some font glyphs implicitly close, e.g. Cantarell-VF.otf
        ttfp::OutlineBuilder::close(&mut self);
        self.outline
    }
}

impl ttfp::OutlineBuilder for OutlineCurveBuilder {
    #[inline]
    fn move_to(&mut self, x: f32, y: f32) {
        // eprintln!("M {x} {y}");
        self.last = point(x, y);
        self.last_move = Some(self.last);
    }

    #[inline]
    fn line_to(&mut self, x1: f32, y1: f32) {
        // eprintln!("L {x1} {y1}");
        let p1 = point(x1, y1);
        self.outline.push(OutlineCurve::Line(self.last, p1));
        self.last = p1;
    }

    #[inline]
    fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        // eprintln!("Q {x1} {y1}");
        let p1 = point(x1, y1);
        let p2 = point(x2, y2);
        self.outline.push(OutlineCurve::Quad(self.last, p1, p2));
        self.last = p2;
    }

    #[inline]
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
        // eprintln!("C {x1} {y1} {x3} {y3}");
        let p1 = point(x1, y1);
        let p2 = point(x2, y2);
        let p3 = point(x3, y3);

        self.outline
            .push(OutlineCurve::Cubic(self.last, p1, p2, p3));
        self.last = p3;
    }

    #[inline]
    fn close(&mut self) {
        // eprintln!("Z");
        if let Some(m) = self.last_move.take() {
            self.outline.push(OutlineCurve::Line(self.last, m));
        }
    }
}

pub struct GlyphPainter<'a> {
    pub canvas: &'a mut image::RgbaImage,
    pub face: &'a ttfp::Face<'a>,
    pub outlined_glyph: ab_glyph::OutlinedGlyph,
    pub scale_factor: ab_glyph::PxScaleFactor,
    pub outline: Vec<OutlineCurve>,
    pub margin: u32,
    pub line: usize,
    pub line_height: u32,
}

impl ttfp::colr::Painter for GlyphPainter<'_> {
    fn outline(&mut self, glyph_id: ttfp::GlyphId) {
        let mut builder = OutlineCurveBuilder::default();
        self.face.outline_glyph(glyph_id, &mut builder);

        self.outline = builder.outline;
    }

    fn paint_foreground(&mut self) {
        // The caller must provide this color. We simply fallback to black.
        self.paint_color(ttfp::RgbaColor::new(0, 0, 0, 255));
    }

    fn paint_color(&mut self, color: ttfp::RgbaColor) {
        let colour = image::Rgba([color.red, color.green, color.blue, color.alpha]);

        let bb = self.outlined_glyph.px_bounds();

        use ab_glyph_rasterizer::Rasterizer;
        let h_factor = self.scale_factor.horizontal;
        let v_factor = -self.scale_factor.vertical;
        let offset = self.outlined_glyph.glyph().position - bb.min;
        let (w, h) = (bb.width() as usize, bb.height() as usize);

        let scale_up = |&Point { x, y }| point(x * h_factor, y * v_factor);

        self.outline
            .iter()
            .fold(Rasterizer::new(w, h), |mut rasterizer, curve| match curve {
                OutlineCurve::Line(p0, p1) => {
                    rasterizer.draw_line(scale_up(p0) + offset, scale_up(p1) + offset);
                    rasterizer
                }
                OutlineCurve::Quad(p0, p1, p2) => {
                    rasterizer.draw_quad(
                        scale_up(p0) + offset,
                        scale_up(p1) + offset,
                        scale_up(p2) + offset,
                    );
                    rasterizer
                }
                OutlineCurve::Cubic(p0, p1, p2, p3) => {
                    rasterizer.draw_cubic(
                        scale_up(p0) + offset,
                        scale_up(p1) + offset,
                        scale_up(p2) + offset,
                        scale_up(p3) + offset,
                    );
                    rasterizer
                }
            })
            .for_each_pixel_2d(|px, py, pv| {
                let px = px + bb.min.x as u32 + self.margin;
                let py = py + bb.min.y as u32 + self.margin + self.line as u32 * self.line_height;

                if self.canvas.in_bounds(px, py) {
                    let pixel = self.canvas.get_pixel(px, py).to_owned();
                    
                    let weighted_color = imageproc::pixelops::interpolate(colour, pixel, pv);
                    self.canvas.put_pixel(px, py, weighted_color);
                }
            });
    }
}
