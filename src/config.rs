#[derive(facet::Facet)]
pub struct Config {
    pub text: String,
    pub margin: u32,
    pub width: u32,

    pub text_color: u32,
    pub bg_color: u32,

    pub font: FontConfig,
}

#[derive(facet::Facet)]
pub struct FontConfig {
    pub path: String,
    pub size: f32,
    pub line_height: f32,

    pub features: Option<Vec<String>>,
    pub variations: Option<Vec<VariationConfig>>,
}

#[derive(facet::Facet)]
pub struct VariationConfig {
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub rest: f32,
}

pub fn read_config(args: &mut pico_args::Arguments) -> Result<Config, Box<dyn std::error::Error>> {
    let config_path = args.opt_value_from_str("--config")?.unwrap_or("nun.toml".to_owned());
    let config_file = std::fs::read_to_string(&config_path)?;

    let config: Config = facet_toml::from_str(&config_file).map_err(|e| e.to_string())?;

    Ok(config)
}
