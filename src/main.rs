macro_rules! my_file {
    () => {
        "kursi"
    };
}

const FACTOR: u32 = 4;

const _WHITE: [u8; 4] = [0xFF; 4];
const _BLACK: [u8; 4] = [0x0A, 0x0A, 0x0A, 0xFF];

const _OFF_WHITE: [u8; 4] = [0xFF, 0xFF, 0xF2, 0xFF];
const _OFF_BLACK: [u8; 4] = [0x20, 0x20, 0x20, 0xFF];

const _MSHQ_DEFAULT: f32 = 25.0;
const _SPAC_DEFAULT: f32 = 0.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = nun::ImageConfig {
        margin: FACTOR * 100,
        img_width: FACTOR * 2000,
        font_size: FACTOR as f32 * 80.0,
        txt_color: _BLACK,
        bkg_color: _OFF_WHITE,
    };

    let variations = [
        nun::Variation::new_spacing(),
        // nun::Variation::new_axis(*b"MSHQ", 0.0, 100.0, _MSHQ_DEFAULT),
        // nun::Variation::new_axis(*b"SPAC", -80.0, 125.0, _SPAC_DEFAULT),
    ];

    nun::run(concat!("texts/", my_file!(), ".txt"), "fonts/NotoArabic.ttf", variations, config)
}
