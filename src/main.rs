mod config;

const FACTOR: u32 = 4;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = pico_args::Arguments::from_env();

    let config = config::read_config(&mut args)?;

    let img_config = nun::ImageConfig {
        margin: FACTOR * config.margin,
        img_width: FACTOR * config.width,
        font_size: FACTOR as f32 * config.font.size,
        line_height: config.font.line_height,
        txt_color: config.text_color.to_be_bytes(),
        bkg_color: config.bg_color.to_be_bytes(),
    };

    let mut variations = config
        .font
        .variations
        .into_iter()
        .flatten()
        .map(|v| nun::Variation::new_axis(tag(v.name), v.min, v.max, v.rest))
        .collect::<Vec<_>>();

    if variations.is_empty() {
        variations.push(nun::Variation::new_spacing());
    }

    let features = config
        .font
        .features
        .into_iter()
        .flatten()
        .map(tag)
        .collect::<Vec<_>>();

    nun::run(
        config.text,
        config.font.path,
        &features,
        variations,
        img_config,
    )
}

fn tag(tag: String) -> [u8; 4] {
    assert!(tag.len() == 4, "Tag length must be 4 bytes");
    tag.as_bytes().try_into().unwrap()
}
