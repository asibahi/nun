use harfbuzz_rs as hb;
use itertools::Itertools as _;
use std::{borrow::Cow, cmp::Ordering, ops::Not};

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
    #[must_use]
    pub fn new(tag: [u8; 4], min: f32, max: f32, best: f32, priority: i32) -> Self {
        Self { tag, min, max, best, current_value: best, priority }
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

    fn change_current_val(&mut self, new_val: f32) {
        self.current_value = new_val;
    }
}

#[derive(Clone, Debug)]
pub struct LineData<const N: usize> {
    pub start_bp: usize,
    pub end_bp: usize,
    pub kashida_locs: Box<[usize]>,
    pub variations: [Variation; N],
}

impl<const N: usize> LineData<N> {
    pub fn new(
        start_bp: usize,
        end_bp: usize,
        kashida_locs: &[usize],
        variations: [Variation; N],
    ) -> Self {
        Self { start_bp, end_bp, kashida_locs: kashida_locs.into(), variations }
    }

    fn cost(&self) -> usize {
        self.variations
            .iter()
            .fold(0, |acc, v| acc + v.cost())
            // figuring out the proper cost function is WIP. I hate inserting kashidas
            .saturating_pow(self.kashida_locs.len() as u32 + 1)
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
            TooTight => write!(f, "Line is too tight."),
            // Maybe => write!(f, "Line is indeterminate."),
            // Impossible => write!(f, "Line is impossible."),
        }
    }
}

pub fn place_kashidas<'a>(text: &'a str, kashida_locs: &'_ [usize]) -> Cow<'a, str> {
    if kashida_locs.is_empty() {
        Cow::Borrowed(text)
    } else {
        let mut buffer = text.to_owned();
        for kc in kashida_locs.iter().sorted_by(|a, b| b.cmp(a)) {
            buffer.insert(*kc, 'ـ');
        }
        Cow::Owned(buffer)
    }
}

fn find_optimal_line_1_axis<const N: usize>(
    hb_font: &mut hb::Font<'_>,
    text: &str,
    (start_bp, end_bp): (usize, usize),
    goal_width: u32,
    kashida_locs: &[usize],
    vv_idx: usize,
    mut variations: [Variation; N],
) -> Result<LineData<N>, LineError<N>> {
    assert!(vv_idx < N, "Index should be within the variations array");

    let ret = LineData::new(start_bp, end_bp, kashida_locs, variations);

    let mut search_range = variations[vv_idx].min..variations[vv_idx].max;

    let text_slice = &place_kashidas(text[start_bp..end_bp].trim(), kashida_locs);

    let mut set_slice_to_axis_value = |val: f32| {
        variations[vv_idx].change_current_val(val);

        let hb_variations = variations
            .iter()
            .map(|v| hb::Variation::new(&v.tag, v.current_value))
            .collect::<Vec<_>>();

        hb_font.set_variations(&hb_variations);

        let buffer = hb::UnicodeBuffer::new().add_str(text_slice);
        let output = hb::shape(hb_font, buffer, &[]);

        let width = output.get_glyph_positions().iter().map(|p| p.x_advance).sum::<i32>() as u32;

        // more lenient searching
        if (goal_width.saturating_sub(5)..goal_width.saturating_add(5)).contains(&width) {
            Ordering::Equal
        } else {
            width.cmp(&goal_width)
        }
    };

    match set_slice_to_axis_value(search_range.start) {
        Ordering::Greater => return Err(LineError::new(TooTight, variations)),
        Ordering::Equal => return Ok(LineData { variations, ..ret }),
        Ordering::Less => (),
    }

    match set_slice_to_axis_value(search_range.end) {
        Ordering::Less => return Err(LineError::new(TooLoose, variations)),
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

fn find_optimal_line<const N: usize>(
    hb_font: &mut hb::Font<'_>,
    full_text: &str,
    (start_bp, end_bp): (usize, usize),
    goal_width: u32,
    mut variations: [Variation; N],
) -> Result<LineData<N>, LineError<N>> {
    assert!(N > 0);

    let kashida_locs =
        kashida::find_kashidas(&full_text[start_bp..end_bp], kashida::Script::Arabic);

    let mut inner = |k| {
        for (idx, counter) in (0..N).rev().enumerate() {
            let attempt = find_optimal_line_1_axis(
                hb_font,
                full_text,
                (start_bp, end_bp),
                goal_width,
                &kashida_locs[0..k],
                idx,
                variations,
            );

            variations = match (attempt, counter) {
                (result @ Ok(_), _) | (result @ Err(_), 0) => return result,
                (Err(LineError { variations, .. }), _) => variations,
            };
        }

        unreachable!("Inner optimal line loop always runs");
    };

    for (k, counter) in (0..=kashida_locs.len()).rev().enumerate() {
        match (inner(k), counter) {
            (result @ Ok(_), _) | (result @ Err(_), 0) => return result,
            (Err(_), _) => (),
        }
    }

    unreachable!("Outer optimal line loop always runs");
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

pub fn line_break<const N: usize>(
    hb_font: &mut hb::Font<'_>,
    text: &str,
    goal_width: u32,
    variations: [Variation; N],
) -> Result<Vec<LineData<N>>, ParagraphError> {
    let mut paragraphs = vec![];

    for paragraph in text.split("\n\n") {
        let line_data = paragraph_line_break(hb_font, text, paragraph, goal_width, variations)?;

        paragraphs.extend(line_data);
    }

    Ok(paragraphs)
}

fn paragraph_line_break<const N: usize>(
    hb_font: &mut hb::Font<'_>,
    full_text: &str,
    paragraph: &str,
    goal_width: u32,
    variations: [Variation; N],
) -> Result<Vec<LineData<N>>, ParagraphError> {
    let start_bp = paragraph.as_ptr() as usize - full_text.as_ptr() as usize;
    let end_bp = start_bp + paragraph.as_bytes().len();

    // first see if the whole paragraph fits in one line
    // for example the Basmala
    if let Ok(l_b) =
        match find_optimal_line(hb_font, full_text, (start_bp, end_bp), goal_width, variations) {
            Ok(data) => Ok(data),
            Err(LineError { kind: TooTight, .. }) => Err(ParagraphError::UnableToLayout),
            Err(LineError { variations, .. }) => {
                Ok(LineData::new(start_bp, end_bp, &[], variations))
            }
        }
    {
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

            match find_optimal_line(hb_font, full_text, (start_bp, end_bp), goal_width, variations)
            {
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
