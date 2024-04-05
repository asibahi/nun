#![allow(unused)]

use harfbuzz_rs as hb;
use itertools::Itertools as _;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt::Debug,
    ops::Not,
};

#[derive(Clone, Copy, Debug)]
pub struct LineData {
    pub start_bp: usize,
    pub end_bp: usize,
    pub variation_value: f32,
}
impl LineData {
    pub fn cost(&self) -> usize {
        f32::abs(self.variation_value - 50.0).powi(2).round() as usize
    }
}

pub const MSHQ: &[u8; 4] = b"MSHQ";
pub const MSHQ_MIN: f32 = 0.0;
pub const MSHQ_MAX: f32 = 100.0;

pub const SPAC: &[u8; 4] = b"SPAC";
pub const SPAC_VAL: f32 = 0.0;

#[derive(Debug)]
pub enum LineError {
    TooLoose,
    TooTight,
    Indeterminate,
}

impl std::error::Error for LineError {}
impl std::fmt::Display for LineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LineError::TooLoose => write!(f, "Line is too loose."),
            LineError::TooTight => write!(f, "Line is too right."),
            LineError::Indeterminate => write!(f, "Line is indeterminate."),
        }
    }
}

pub fn find_optimal_line(
    hb_font: &mut hb::Font<'_>,
    text: &str,
    start_bp: usize,
    end_bp: usize,
    desired_width: u32,
    scale_factor: f32,
) -> Result<LineData, LineError> {
    let ret = LineData {
        start_bp,
        end_bp,
        variation_value: 50.0,
    };

    let mut search_range = MSHQ_MIN..MSHQ_MAX;

    let slice = &text[start_bp..end_bp];

    let mut set_slice_to_axis_value = |val: f32| {
        hb_font.set_variations(&[
            hb::Variation::new(MSHQ, val),
            hb::Variation::new(SPAC, SPAC_VAL),
        ]);

        let buffer = hb::UnicodeBuffer::new().add_str_item(text, slice.trim());

        let output = hb::shape(hb_font, buffer, &[]);
        let width: i32 = output
            .get_glyph_positions()
            .iter()
            .map(|p| p.x_advance)
            .sum();

        width as f32 * scale_factor
    };

    let end_test = set_slice_to_axis_value(search_range.end).round() as u32;

    match end_test.cmp(&desired_width) {
        Ordering::Less => return Err(LineError::TooLoose),
        Ordering::Equal => {
            return Ok(LineData {
                variation_value: search_range.end,
                ..ret
            })
        }
        Ordering::Greater => (),
    }

    let start_test = set_slice_to_axis_value(search_range.start).round() as u32;
    match start_test.cmp(&desired_width) {
        Ordering::Greater => return Err(LineError::TooTight),
        Ordering::Equal => {
            return Ok(LineData {
                variation_value: search_range.start,
                ..ret
            })
        }
        Ordering::Less => (),
    }

    if start_test == end_test {
        return Err(LineError::Indeterminate);
    }

    let mut prev_test = None;

    for _ in 0..30 {
        let mid = (search_range.start + search_range.end) / 2.0;
        let test = set_slice_to_axis_value(mid).round() as u32;

        if Some(test) == prev_test {
            println!("indetemrinate");
            return Ok(LineData {
                variation_value: mid,
                ..ret
            });
        }

        search_range = match test.cmp(&desired_width) {
            Ordering::Less => mid..search_range.end,
            Ordering::Equal => {
                return Ok(LineData {
                    variation_value: mid,
                    ..ret
                })
            }
            Ordering::Greater => search_range.start..mid,
        };

        prev_test = Some(test);
    }

    Err(LineError::Indeterminate)
}

pub fn line_break(
    hb_font: &mut hb::Font<'_>,
    text: &str,
    desired_width: u32,
    scale_factor: f32,
) -> Result<Vec<LineData>, Box<dyn std::error::Error>> {
    let segmenter = icu_segmenter::LineSegmenter::new_auto();
    let bps = segmenter.segment_str(text).collect::<Vec<_>>();

    let mut nodes = HashSet::new();
    nodes.insert(0);

    let mut edges: HashMap<(usize, usize), LineData> = HashMap::new();

    for i in 0..bps.len() {
        if nodes.contains(&bps[i]).not() {
            continue;
        }

        for j in (i..bps.len()).skip(1) {
            let i = bps[i];
            let j = bps[j];
            let attempt = find_optimal_line(hb_font, text, i, j, desired_width, scale_factor);

            match attempt {
                Err(LineError::TooLoose) => continue,
                Err(LineError::TooTight) => break,

                // Not sure how to deal with this for now
                Err(LineError::Indeterminate) => {
                    todo!("Line is indeterminate at ({i}, {j}): {}", &text[i..j])
                }

                Ok(data) => {
                    nodes.insert(j);
                    edges.insert((i, j), data);
                }
            }
        }
    }

    let (shortest_path, _) = pathfinding::prelude::dijkstra(
        &bps[0],
        |i| {
            edges
                .iter()
                .filter(|((ki, _), _)| ki == i)
                .map(|((_, kj), v)| (*kj, v.cost()))
                .collect::<Vec<_>>()
        },
        |p| p == bps.last().unwrap(),
    )
    .ok_or::<&str>("Couldn't find path")?;

    let lines = shortest_path
        .into_iter()
        .tuple_windows()
        .map(|key| edges[&key])
        .inspect(|ln| {
            dbg!(&ln);
        })
        .collect::<Vec<_>>();

    Ok(lines)
}
