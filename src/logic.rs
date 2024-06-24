use crate::shaper::Shaper;
use itertools::Itertools as _;
use std::{cmp::Ordering, ops::Not};

#[derive(Clone, Copy, Debug)]
pub struct Variation {
    pub kind: VariationKind,
    pub current_value: f32,

    min: f32,
    max: f32,

    best: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum VariationKind {
    Axis([u8; 4]),
    Spacing,
}

impl Variation {
    #[must_use]
    pub fn new_spacing() -> Self {
        Self {
            kind: VariationKind::Spacing,
            min: 0.25, // I dunno
            max: 1.25, // maybe?
            best: 1.0,
            current_value: 1.0,
        }
    }

    #[must_use]
    pub fn new_axis(tag: [u8; 4], min: f32, max: f32, best: f32) -> Self {
        Self { kind: VariationKind::Axis(tag), min, max, best, current_value: best }
    }

    // lower priority is lower cost (i.e. better)
    fn cost(&self, priority: usize) -> usize {
        // normalizes difference between current_value and best
        let dif = (self.current_value - self.best) * 100.0 / (self.max - self.min);
        dif.abs().powi(priority as i32 + 2) as usize
    }

    fn change_current_val(&mut self, new_val: f32) {
        self.current_value = new_val;
    }
}

#[derive(Clone, Debug)]
pub struct LineData<const N: usize> {
    pub start_bp: usize,
    pub end_bp: usize,
    pub variations: [Variation; N],
    pub kashida_count: usize,
}

impl<const N: usize> LineData<N> {
    pub fn new(
        start_bp: usize,
        end_bp: usize,
        variations: [Variation; N],
        kashida_count: usize,
    ) -> Self {
        Self { start_bp, end_bp, variations, kashida_count }
    }

    pub(crate) fn cost(&self) -> usize {
        let k_v = Variation {
            kind: VariationKind::Spacing,
            current_value: self.kashida_count as f32,
            min: 0.0,
            max: 100.0,
            best: 0.0,
        }
        .cost(N);

        self.variations.iter().enumerate().fold(k_v, |acc, (i, v)| acc + v.cost(i))
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
    kind: LineErrorKind,
    variations: [Variation; N],
    kashida_count: usize,
}

impl<const N: usize> LineError<N> {
    fn new(kind: LineErrorKind, variations: [Variation; N], kashida_count: usize) -> Self {
        Self { kind, variations, kashida_count }
    }
}
impl<const N: usize> std::error::Error for LineError<N> {}
impl<const N: usize> std::fmt::Display for LineError<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            TooLoose => write!(f, "Line is too loose."),
            TooTight => write!(f, "Line is too tight."),
            // Maybe => write!(f, "Line is indeterminate."),
            // Impossible => write!(f, "Line is impossible."),
        }
    }
}

fn find_optimal_line_1_axis<'a, const N: usize>(
    shaper_font: &mut impl Shaper<'a>,
    text: &str,
    (start_bp, end_bp): (usize, usize),
    goal_width: u32,
    vv_idx: usize,
    mut variations: [Variation; N],
    (kashida_locs, kashida_count): (&[usize], usize),
) -> Result<LineData<N>, LineError<N>> {
    assert!(vv_idx < N, "Index should be within the variations array");

    let ret = LineData::new(start_bp, end_bp, variations, kashida_count);

    let mut search_range = variations[vv_idx].min..variations[vv_idx].max;

    let text_slice = // if kashida_locs is empty this is a noop.
        kashida::place_kashidas(text[start_bp..end_bp].trim(), kashida_locs, kashida_count);

    let mut set_slice_to_axis_value = |val: f32| {
        variations[vv_idx].change_current_val(val);

        let shaped_text = shaper_font.shape_text(&text_slice, &variations);

        let width = shaped_text.iter().map(|g| g.x_advance).sum::<i32>() as u32;

        if (goal_width.saturating_sub(5)..goal_width.saturating_add(5)).contains(&width) {
            Ordering::Equal
        } else {
            width.cmp(&goal_width)
        }
    };

    match set_slice_to_axis_value(search_range.start) {
        Ordering::Greater => return Err(LineError::new(TooTight, variations, kashida_count)),
        Ordering::Equal => return Ok(LineData { variations, ..ret }),
        Ordering::Less => (),
    }

    match set_slice_to_axis_value(search_range.end) {
        Ordering::Less => return Err(LineError::new(TooLoose, variations, kashida_count)),
        Ordering::Equal => return Ok(LineData { variations, ..ret }),
        Ordering::Greater => (),
    }

    // What to do if variations do not change the line's width?
    // Open question for another font !!

    let mut i = 0;
    loop {
        let mid = (search_range.start + search_range.end) / 2.0;

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

fn find_optimal_line<'a, const N: usize>(
    shaper_font: &mut impl Shaper<'a>,
    full_text: &str,
    (start_bp, end_bp): (usize, usize),
    goal_width: u32,
    variations: [Variation; N],
    kashida: bool,
) -> Result<LineData<N>, LineError<N>> {
    const { assert!(N > 0) }

    let mut inner = |k| {
        let mut variations = variations;
        for (idx, counter) in (0..N).rev().enumerate() {
            let attempt = find_optimal_line_1_axis(
                shaper_font,
                full_text,
                (start_bp, end_bp),
                goal_width,
                idx,
                variations,
                k,
            );

            variations = match (attempt, counter) {
                (result @ Ok(_), _)
                | (result @ Err(LineError { kind: TooTight, .. }), _)
                | (result @ Err(_), 0) => return result,
                (Err(LineError { variations, .. }), _) => variations,
            };
        }

        unreachable!("Inner optimal line loop always runs");
    };

    if kashida {
        let locs =
            kashida::find_kashidas(full_text[start_bp..end_bp].trim(), kashida::Script::Arabic);
        for (k, counter) in (0..=locs.len() * 20).rev().enumerate() {
            match (inner((&locs, k)), counter) {
                (result @ Ok(_), _)
                | (result @ Err(LineError { kind: TooTight, .. }), _)
                | (result @ Err(_), 0) => return result,
                (Err(_), _) => (),
            }
        }

        unreachable!()
    } else {
        inner((&[], 0))
    }
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

pub fn line_break<'a, const N: usize>(
    shaper_font: &mut impl Shaper<'a>,
    text: &str,
    goal_width: u32,
    variations: [Variation; N],
) -> Result<Vec<LineData<N>>, ParagraphError> {
    let mut paragraphs = vec![];

    for paragraph in text.split("\n\n") {
        let line_data = if let Ok(line_data) =
            paragraph_line_break(shaper_font, text, paragraph, goal_width, variations, false)
        {
            line_data
        } else {
            paragraph_line_break(shaper_font, text, paragraph, goal_width, variations, true)?
        };
        paragraphs.extend(line_data)
    }

    Ok(paragraphs)
}

fn paragraph_line_break<'a, const N: usize>(
    shaper_font: &mut impl Shaper<'a>,
    full_text: &str,
    paragraph: &str,
    goal_width: u32,
    variations: [Variation; N],
    kashida: bool,
) -> Result<Vec<LineData<N>>, ParagraphError> {
    let start_bp = paragraph.as_ptr() as usize - full_text.as_ptr() as usize;
    let end_bp = start_bp + paragraph.as_bytes().len();

    // first see if the whole paragraph fits in one line
    // for example the Basmala
    if let Ok(l_b) = match find_optimal_line(
        shaper_font,
        full_text,
        (start_bp, end_bp),
        goal_width,
        variations,
        true,
    ) {
        Ok(data) => Ok(data),
        Err(LineError { kind: TooTight, .. }) => Err(ParagraphError::UnableToLayout),
        Err(LineError { variations, kashida_count, .. }) => {
            Ok(LineData::new(start_bp, end_bp, variations, kashida_count))
        }
    } {
        return Ok(vec![l_b]);
    }

    let bps = icu_segmenter::LineSegmenter::new_auto()
        .segment_str(paragraph)
        .map(|bp| bp + start_bp)
        .collect::<Vec<_>>();

    let mut nodes = hashbrown::HashSet::new();
    nodes.insert(start_bp);

    let mut edges = hashbrown::HashMap::new();

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
                shaper_font,
                full_text,
                (start_bp, end_bp),
                goal_width,
                variations,
                kashida,
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
            .map(|key: (_, _)| edges.get(&key).cloned())
            .collect::<Option<Vec<_>>>()
    })
    .ok_or(ParagraphError::UnableToLayout)
}
