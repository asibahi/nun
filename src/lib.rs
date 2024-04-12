use harfbuzz_rs as hb;
use itertools::Itertools as _;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt::Debug,
    ops::Not,
};

#[derive(Clone, Copy, Debug)]
pub struct Variation {
    pub tag: [u8; 4],
    pub current_value: f32,

    min: f32,
    max: f32,

    best: f32,

    /// lower is better.
    priority: i32,
}

impl Variation {
    pub fn new(tag: [u8; 4], min: f32, max: f32, best: f32, priority: i32) -> Self {
        Self {
            tag,
            min,
            max,
            best,
            current_value: best,
            priority,
        }
    }

    pub fn set_variations<const N: usize>(
        variations: [Variation; N],
        ab_font: &mut impl ab_glyph::VariableFont,
        hb_font: &mut hb::Owned<hb::Font<'_>>,
    ) {
        hb_font.set_variations(&variations.map(|v| hb::Variation::new(&v.tag, v.current_value)));

        for v in variations {
            ab_font.set_variation(&v.tag, v.current_value);
        }
    }

    fn cost(&self) -> usize {
        f32::abs(self.current_value - self.best).powi(self.priority + 2) as usize
    }

    fn change_current_val(self, new_val: f32) -> Self {
        Self {
            current_value: new_val,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LineData<const N: usize> {
    pub start_bp: usize,
    pub end_bp: usize,
    pub variations: [Variation; N],
    pub last_line: bool,
}

impl<const N: usize> LineData<N> {
    pub fn new(start_bp: usize, end_bp: usize, variations: [Variation; N]) -> Self {
        Self {
            start_bp,
            end_bp,
            variations,
            last_line: false,
        }
    }

    fn cost(&self) -> usize {
        self.variations
            .iter()
            .map(|v| v.cost())
            .reduce(std::ops::Add::add)
            .unwrap_or(usize::MAX)
    }
}

#[derive(Debug)]
enum LineErrorKind {
    TooLoose,
    TooTight,
    Maybe,
    // Impossible,
}
use LineErrorKind::*;

#[derive(Debug)]
struct LineError {
    variation: Variation,
    kind: LineErrorKind,
}

impl LineError {
    fn new(kind: LineErrorKind, variation: Variation) -> Self {
        Self { variation, kind }
    }
}
impl std::error::Error for LineError {}
impl std::fmt::Display for LineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            TooLoose => write!(f, "Line is too loose."),
            TooTight => write!(f, "Line is too right."),
            Maybe => write!(f, "Line is indeterminate."),
            // Impossible => write!(f, "Line is impossible."),
        }
    }
}

fn find_optimal_line(
    hb_font: &mut hb::Font<'_>,
    text: &str,
    (start_bp, end_bp): (usize, usize),
    goal_width: u32,
    variable_variation: Variation,
    fixed_variation: Variation,
) -> Result<LineData<2>, LineError> {
    let ret = LineData::new(start_bp, end_bp, [variable_variation, fixed_variation]);

    let mut search_range = variable_variation.min..variable_variation.max;

    let slice = &text[start_bp..end_bp];

    let mut set_slice_to_axis_value = |val: f32| {
        hb_font.set_variations(&[
            hb::Variation::new(&variable_variation.tag, val),
            hb::Variation::new(&fixed_variation.tag, fixed_variation.current_value),
        ]);

        let buffer = hb::UnicodeBuffer::new().add_str_item(text, slice.trim());
        let output = hb::shape(hb_font, buffer, &[]);

        let width: i32 = output
            .get_glyph_positions()
            .iter()
            .map(|p| p.x_advance)
            .sum();

        width as u32
    };

    let start_test = set_slice_to_axis_value(search_range.start);
    let start_variation = variable_variation.change_current_val(search_range.start);
    match start_test.cmp(&goal_width) {
        Ordering::Greater => return Err(LineError::new(TooTight, start_variation)),
        Ordering::Equal => {
            return Ok(LineData {
                variations: [start_variation, fixed_variation],
                ..ret
            })
        }
        Ordering::Less => (),
    }

    let end_test = set_slice_to_axis_value(search_range.end);
    let end_variation = variable_variation.change_current_val(search_range.end);
    match end_test.cmp(&goal_width) {
        Ordering::Less => return Err(LineError::new(TooLoose, end_variation)),
        Ordering::Equal => {
            return Ok(LineData {
                variations: [end_variation, fixed_variation],
                ..ret
            })
        }
        Ordering::Greater => (),
    }

    if start_test == end_test {
        return Err(LineError::new(Maybe, start_variation));
    }

    let mut prev_test = None;

    let mut i = 0;
    loop {
        let mid = (search_range.start + search_range.end) / 2.0;
        let mid_variation = variable_variation.change_current_val(mid);

        let test = set_slice_to_axis_value(mid);

        if i == 30 || Some(test) == prev_test {
            return Ok(LineData {
                variations: [mid_variation, fixed_variation],
                ..ret
            });
        }

        search_range = match test.cmp(&goal_width) {
            Ordering::Less => mid..search_range.end,
            Ordering::Equal => {
                return Ok(LineData {
                    variations: [mid_variation, fixed_variation],
                    ..ret
                })
            }
            Ordering::Greater => search_range.start..mid,
        };

        i += 1;
        prev_test = Some(test);
    }
}

#[derive(Debug)]
pub enum PageError {
    UnableToLayout,
}
impl std::error::Error for PageError {}
impl std::fmt::Display for PageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PageError::UnableToLayout => write!(f, "Unable to layout page."),
        }
    }
}

pub fn line_break(
    hb_font: &mut hb::Font<'_>,
    text: &str,
    goal_width: u32,
    primary_variation: Variation,
    secondary_variation: Variation,
) -> Result<Vec<LineData<2>>, PageError> {
    let mut paragraphs = vec![];

    for paragraph in text.split("\n\n") {
        let line_data = paragraph_line_break(
            hb_font,
            text,
            paragraph,
            goal_width,
            primary_variation,
            secondary_variation,
        )?;

        paragraphs.extend(line_data);
    }

    Ok(paragraphs)
}

fn single_line_paragraph(
    hb_font: &mut hb::Font<'_>,
    full_text: &str,
    paragraph: &str,
    goal_width: u32,
    primary_variation: Variation,
    secondary_variation: Variation,
) -> Result<LineData<2>, PageError> {
    let start_bp = paragraph.as_ptr() as usize - full_text.as_ptr() as usize;
    let end_bp = start_bp + paragraph.as_bytes().len();
    match find_optimal_line(
        hb_font,
        full_text,
        (start_bp, end_bp),
        goal_width,
        primary_variation,
        secondary_variation,
    ) {
        Ok(data) => Ok(data),
        Err(LineError {
            variation,
            kind: TooLoose,
        }) => Ok(LineData {
            start_bp,
            end_bp,
            variations: [variation, secondary_variation],
            last_line: true,
        }),
        Err(LineError { variation, .. }) => {
            match find_optimal_line(
                hb_font,
                full_text,
                (start_bp, end_bp),
                goal_width,
                secondary_variation,
                variation,
            ) {
                Ok(data) => Ok(data),
                Err(LineError { kind: TooTight, .. }) => Err(PageError::UnableToLayout),
                Err(LineError { variation, .. }) => Ok(LineData {
                    start_bp: 0,
                    end_bp,
                    variations: [variation, secondary_variation],
                    last_line: true,
                }),
            }
        }
    }
}

fn paragraph_line_break(
    hb_font: &mut hb::Font<'_>,
    full_text: &str,
    paragraph: &str,
    goal_width: u32,
    primary_variation: Variation,
    secondary_variation: Variation,
) -> Result<Vec<LineData<2>>, PageError> {
    // first see if the whole paragraph fits in one line
    // for example the Basmala
    if let Ok(l_b) = single_line_paragraph(
        hb_font,
        full_text,
        paragraph,
        goal_width,
        primary_variation,
        secondary_variation,
    ) {
        return Ok(vec![l_b]);
    }

    let bps = icu_segmenter::LineSegmenter::new_auto()
        .segment_str(paragraph)
        .map(|bp| bp + (paragraph.as_ptr() as usize - full_text.as_ptr() as usize))
        .collect::<Vec<_>>();

    let mut nodes = HashSet::new();
    nodes.insert(bps[0]);

    let mut edges: HashMap<(usize, usize), LineData<2>> = HashMap::new();

    for i in 0..bps.len() {
        if nodes.contains(&bps[i]).not() {
            continue;
        }

        for j in (i..bps.len()).skip(1) {
            let start_bp = bps[i];
            let end_bp = bps[j];
            let fst_try = find_optimal_line(
                hb_font,
                full_text,
                (start_bp, end_bp),
                goal_width,
                primary_variation,
                secondary_variation,
            );

            let (nearest_variation, fst_err) = match fst_try {
                Ok(data) => {
                    nodes.insert(end_bp);
                    edges.insert((start_bp, end_bp), data);
                    continue;
                }
                Err(LineError { variation, kind }) => (variation, kind),
            };

            let snd_try = find_optimal_line(
                hb_font,
                full_text,
                (start_bp, end_bp),
                goal_width,
                secondary_variation,
                nearest_variation,
            );

            let snd_err = match snd_try {
                Ok(data) => {
                    nodes.insert(end_bp);
                    edges.insert((start_bp, end_bp), data);
                    continue;
                }
                Err(LineError { kind, .. }) => kind,
            };

            match (fst_err, snd_err) {
                (TooTight, TooTight) => break,
                (TooLoose, _) | (_, TooLoose) => continue,
                (Maybe, _) | (_, Maybe) => {
                    unimplemented!(
                        "No idea what to do. Line is indeterminate at ({start_bp}, {end_bp}): {}",
                        &full_text[start_bp..end_bp]
                    )
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
    .ok_or(PageError::UnableToLayout)?;

    let mut lines = shortest_path
        .into_iter()
        .tuple_windows()
        .map(|key| edges[&key])
        .collect::<Vec<_>>();
    if let Some(ld) = lines.last_mut() {
        ld.last_line = true;
    }

    Ok(lines)
}
