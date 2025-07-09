mod config;

const FACTOR: u32 = 4;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = pico_args::Arguments::from_env();

    let config = config::read_config(&mut args)?;

    let img_config = nun::ImageConfig {
        margin: FACTOR * config.margin,
        img_width: FACTOR * config.width,
        font_size: FACTOR as f32 * config.font.size,
        txt_color: config.text_color.to_be_bytes(),
        bkg_color: config.bg_color.to_be_bytes(),
    };

    let variations = if config.font.variations.is_empty() {
        vec![nun::Variation::new_spacing()]
    } else {
        config
            .font
            .variations
            .into_iter()
            .map(|v| {
                assert!(v.name.len() == 4);
                let axis: [u8; 4] = v.name.as_bytes().try_into().unwrap();
                nun::Variation::new_axis(axis, v.min, v.max, v.rest)
            })
            .collect()
    };

    nun::run(config.text, config.font.path, variations, img_config)
}
