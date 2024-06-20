use itertools::Itertools as _;
use rustybuzz as rb;
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
        Self { tag, min, max, best, current_value: best, priority }
    }

    pub fn set_variations<const N: usize>(
        variations: [Variation; N],
        ab_font: &mut impl ab_glyph::VariableFont,
        rb_font: &mut rb::Face<'_>,
    ) {
        rb_font.set_variations(&variations.map(|v| rb::Variation {
            tag: rb::ttf_parser::Tag::from_bytes(&v.tag),
            value: v.current_value,
        }));

        for v in variations {
            ab_font.set_variation(&v.tag, v.current_value);
        }
    }

    fn cost(&self) -> usize {
        f32::abs(self.current_value - self.best).powi(self.priority + 2) as usize
    }

    fn change_current_val(self, new_val: f32) -> Self {
        Self { current_value: new_val, ..self }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LineData<const N: usize> {
    pub start_bp: usize,
    pub end_bp: usize,
    pub variations: [Variation; N],
}

impl<const N: usize> LineData<N> {
    pub fn new(start_bp: usize, end_bp: usize, variations: [Variation; N]) -> Self {
        Self { start_bp, end_bp, variations }
    }

    fn cost(&self) -> usize {
        self.variations.iter().map(Variation::cost).reduce(std::ops::Add::add).unwrap_or(usize::MAX)
    }
}

#[derive(Debug)]
enum LineErrorKind {
    TooLoose,
    TooTight,
    // Maybe,
    // Impossible,
}
use LineErrorKind::{TooLoose, TooTight};

#[derive(Debug)]
struct LineError<const N: usize> {
    variations: [Variation; N],
    kind: LineErrorKind,
}

impl<const N: usize> LineError<N> {
    fn new(kind: LineErrorKind, variations: [Variation; N]) -> Self {
        Self { variations, kind }
    }
}
impl<const N: usize> std::error::Error for LineError<N> {}
impl<const N: usize> std::fmt::Display for LineError<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            TooLoose => write!(f, "Line is too loose."),
            TooTight => write!(f, "Line is too right."),
            // Maybe => write!(f, "Line is indeterminate."),
            // Impossible => write!(f, "Line is impossible."),
        }
    }
}

fn find_optimal_line_1_axis(
    rb_font: &mut rb::Face<'_>,
    text: &str,
    (start_bp, end_bp): (usize, usize),
    goal_width: u32,
    variable_variation: Variation,
    fixed_variation: Variation,
) -> Result<LineData<2>, LineError<2>> {
    let ret = LineData::new(start_bp, end_bp, [variable_variation, fixed_variation]);

    let mut search_range = variable_variation.min..variable_variation.max;

    let mut set_slice_to_axis_value = |value: f32| {
        rb_font.set_variations(&[
            rb::Variation { tag: rb::ttf_parser::Tag::from_bytes(&variable_variation.tag), value },
            rb::Variation { tag: rb::ttf_parser::Tag::from_bytes(&fixed_variation.tag), value },
        ]);

        let mut buffer = rb::UnicodeBuffer::new();
        buffer.push_str(text[start_bp..end_bp].trim());
        // buffer.guess_segment_properties(); // do I need this?

        let output = rb::shape(rb_font, &[], buffer);

        let width = output.glyph_positions().iter().map(|p| p.x_advance).sum::<i32>() as u32;

        // more lenient searching
        if (goal_width.saturating_sub(5)..goal_width.saturating_add(5)).contains(&width) {
            Ordering::Equal
        } else {
            width.cmp(&goal_width)
        }
    };

    let variations = [variable_variation.change_current_val(search_range.start), fixed_variation];
    match set_slice_to_axis_value(search_range.start) {
        Ordering::Greater => return Err(LineError::new(TooTight, variations)),
        Ordering::Equal => return Ok(LineData { variations, ..ret }),
        Ordering::Less => (),
    }

    let variations = [variable_variation.change_current_val(search_range.end), fixed_variation];
    match set_slice_to_axis_value(search_range.end) {
        Ordering::Less => return Err(LineError::new(TooLoose, variations)),
        Ordering::Equal => return Ok(LineData { variations, ..ret }),
        Ordering::Greater => (),
    }

    // What to do if variations do not change the line's width?
    // Open question for another font !!
    // if start_test_width == end_test_width {
    //     return Err(LineError::new(Maybe, start_variation));
    // }

    let mut i = 0;
    loop {
        let mid = (search_range.start + search_range.end) / 2.0;
        let variations = [variable_variation.change_current_val(mid), fixed_variation];

        if i >= 30 {
            return Ok(LineData { variations, ..ret });
        }

        search_range = match set_slice_to_axis_value(mid) {
            Ordering::Less => mid..search_range.end,
            Ordering::Equal => return Ok(LineData { variations, ..ret }),
            Ordering::Greater => search_range.start..mid,
        };

        i += 1;
    }
}

fn find_optimal_line(
    rb_font: &mut rb::Face<'_>,
    full_text: &str,
    start_bp: usize,
    end_bp: usize,
    goal_width: u32,
    primary_variation: Variation,
    secondary_variation: Variation,
) -> Result<LineData<2>, LineError<2>> {
    let fst_try = find_optimal_line_1_axis(
        rb_font,
        full_text,
        (start_bp, end_bp),
        goal_width,
        primary_variation,
        secondary_variation,
    );

    let nearest_variation = match fst_try {
        Ok(data) => return Ok(data),
        Err(LineError { variations, .. }) => variations,
    };

    find_optimal_line_1_axis(
        rb_font,
        full_text,
        (start_bp, end_bp),
        goal_width,
        secondary_variation,
        nearest_variation[0],
    )
}

#[derive(Debug)]
pub enum ParagraphError {
    UnableToLayout,
}
impl std::error::Error for ParagraphError {}
impl std::fmt::Display for ParagraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParagraphError::UnableToLayout => write!(f, "Unable to layout paragraph."),
        }
    }
}

pub fn line_break(
    rb_font: &mut rb::Face<'_>,
    text: &str,
    goal_width: u32,
    primary_variation: Variation,
    secondary_variation: Variation,
) -> Result<Vec<LineData<2>>, ParagraphError> {
    let mut paragraphs = vec![];

    for paragraph in text.split("\n\n") {
        let line_data = paragraph_line_break(
            rb_font,
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

fn paragraph_line_break(
    rb_font: &mut rb::Face<'_>,
    full_text: &str,
    paragraph: &str,
    goal_width: u32,
    primary_variation: Variation,
    secondary_variation: Variation,
) -> Result<Vec<LineData<2>>, ParagraphError> {
    let start_bp = paragraph.as_ptr() as usize - full_text.as_ptr() as usize;
    let end_bp = start_bp + paragraph.as_bytes().len();

    // first see if the whole paragraph fits in one line
    // for example the Basmala
    if let Ok(l_b) = match find_optimal_line(
        rb_font,
        full_text,
        start_bp,
        end_bp,
        goal_width,
        primary_variation,
        secondary_variation,
    ) {
        Ok(data) => Ok(data),
        Err(LineError { kind: TooTight, .. }) => Err(ParagraphError::UnableToLayout),
        Err(LineError { variations, .. }) => Ok(LineData { start_bp, end_bp, variations }),
    } {
        return Ok(vec![l_b]);
    }

    let bps = icu_segmenter::LineSegmenter::new_auto()
        .segment_str(paragraph)
        .map(|bp| bp + start_bp)
        .collect::<Vec<_>>();

    let mut nodes = HashSet::new();
    nodes.insert(start_bp);

    let mut edges: HashMap<(usize, usize), LineData<2>> = HashMap::new();

    for i in 0..bps.len() {
        if nodes.contains(&bps[i]).not() {
            continue;
        }

        for j in (i..bps.len()).skip(1) {
            let start_bp = bps[i];
            let end_bp = bps[j];

            if full_text[end_bp..].chars().next().is_some_and(|c| c == '۝') {
                // avoid lines starting with Aya markers
                continue;
            }

            match find_optimal_line(
                rb_font,
                full_text,
                start_bp,
                end_bp,
                goal_width,
                primary_variation,
                secondary_variation,
            ) {
                Ok(data) => {
                    nodes.insert(end_bp);
                    edges.insert((start_bp, end_bp), data);
                }
                Err(LineError { kind: TooTight, .. }) => break,
                _ => (),
            }
        }
    }

    pathfinding::prelude::dijkstra(
        &start_bp,
        |&p| edges.iter().filter_map(move |(&(s, e), ld)| s.eq(&p).then(|| (e, ld.cost()))),
        |&p| p == end_bp,
    )
    .and_then(|(path, _)| {
        path.into_iter()
            .tuple_windows()
            .map(|key| edges.get(&key).copied())
            .collect::<Option<Vec<_>>>()
    })
    .ok_or(ParagraphError::UnableToLayout)
}
