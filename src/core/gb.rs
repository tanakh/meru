use anyhow::{bail, Result};
use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use log::log_enabled;
use serde::{Deserialize, Serialize};
use std::{
    cmp::min,
    fs::File,
    io,
    path::{Path, PathBuf},
};

use tgbr_core::{AudioBuffer, BootRoms, Color, GameBoy, Input as GBInput, Model, Pad, Rom};

use crate::{
    core::{CoreInfo, EmulatorCore, KeyConfig},
    key_assign::*,
};

pub struct GameBoyCore {
    gb: GameBoy,
    config: GameBoyConfig,
    frame_buffer: super::FrameBuffer,
    audio_buffer: super::AudioBuffer,
}

const CORE_INFO: CoreInfo = CoreInfo {
    system_name: "GameBoy (TGBR)",
    abbrev: "gb",
    file_extensions: &["gb", "gbc"],
};

#[derive(Clone, Serialize, Deserialize)]
pub struct GameBoyConfig {
    model: Model,
    boot_rom: BootRom,
    custom_boot_roms: CustomBootRoms,
    palette: PaletteSelect,
    color_correction: bool,
    key_config: KeyConfig,
}

fn default_key_config() -> KeyConfig {
    #[rustfmt::skip]
    let keys = vec![
        ("up", any!(keycode!(Up), pad_button!(0, DPadUp))),
        ("down", any!(keycode!(Down), pad_button!(0, DPadDown))),
        ("left", any!(keycode!(Left), pad_button!(0, DPadLeft))),
        ("right", any!(keycode!(Right), pad_button!(0, DPadRight))),
        ("a", any!(keycode!(X), pad_button!(0, South))),
        ("b", any!(keycode!(Z), pad_button!(0, West))),
        ("start", any!(keycode!(Return), pad_button!(0, Start))),
        ("select", any!(keycode!(RShift), pad_button!(0, Select))),
    ];

    KeyConfig {
        keys: keys.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    }
}

impl Default for GameBoyConfig {
    fn default() -> Self {
        Self {
            model: Model::Auto,
            boot_rom: BootRom::Internal,
            custom_boot_roms: CustomBootRoms::default(),
            palette: PaletteSelect::Pocket,
            color_correction: true,
            key_config: default_key_config(),
        }
    }
}

#[rustfmt::skip]
const BOOT_ROMS: &[(&str, &[u8])] = &[
    ("DMG", include_bytes!("../../assets/sameboy-bootroms/dmg_boot.bin")),
    ("CGB", include_bytes!("../../assets/sameboy-bootroms/cgb_boot.bin")),
    ("SGB", include_bytes!("../../assets/sameboy-bootroms/sgb_boot.bin")),
    ("SGB2",include_bytes!("../../assets/sameboy-bootroms/sgb2_boot.bin")),
    ("AGB", include_bytes!("../../assets/sameboy-bootroms/agb_boot.bin")),
];

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootRom {
    None,
    Internal,
    Custom,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct CustomBootRoms {
    pub dmg: Option<PathBuf>,
    pub cgb: Option<PathBuf>,
    // pub sgb: Option<PathBuf>,
    // pub sgb2: Option<PathBuf>,
    // pub agb: Option<PathBuf>,
}

pub type Palette = [Color; 4];

pub const PALETTE_DMG: Palette = [
    Color::new(120, 128, 16),
    Color::new(92, 120, 64),
    Color::new(56, 88, 76),
    Color::new(40, 64, 56),
];

pub const PALETTE_POCKET: Palette = [
    Color::new(200, 200, 168),
    Color::new(164, 164, 140),
    Color::new(104, 104, 84),
    Color::new(40, 40, 20),
];

pub const PALETTE_LIGHT: Palette = [
    Color::new(0, 178, 132),
    Color::new(0, 156, 116),
    Color::new(0, 104, 74),
    Color::new(0, 80, 56),
];

pub const PALETTE_GRAYSCALE: Palette = [
    Color::new(255, 255, 255),
    Color::new(170, 170, 170),
    Color::new(85, 85, 85),
    Color::new(0, 0, 0),
];

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteSelect {
    Dmg,
    Pocket,
    Light,
    Grayscale,
    Custom(Palette),
}

impl PaletteSelect {
    pub fn get_palette(&self) -> &Palette {
        match self {
            PaletteSelect::Dmg => &PALETTE_DMG,
            PaletteSelect::Pocket => &PALETTE_POCKET,
            PaletteSelect::Light => &PALETTE_LIGHT,
            PaletteSelect::Grayscale => &PALETTE_GRAYSCALE,
            PaletteSelect::Custom(pal) => pal,
        }
    }
}

impl GameBoyConfig {
    pub fn boot_roms(&self) -> BootRoms {
        match self.boot_rom {
            BootRom::None => BootRoms::default(),
            BootRom::Internal => {
                let lookup = |name: &str| {
                    BOOT_ROMS
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, b)| b.to_vec())
                };
                BootRoms {
                    dmg: lookup("DMG"),
                    cgb: lookup("CGB"),
                    sgb: lookup("SGB"),
                    sgb2: lookup("SGB2"),
                    agb: lookup("AGB"),
                }
            }
            BootRom::Custom => {
                let load =
                    |path: &Option<PathBuf>| path.as_ref().map(|path| std::fs::read(path).unwrap());
                BootRoms {
                    dmg: load(&self.custom_boot_roms.dmg),
                    cgb: load(&self.custom_boot_roms.cgb),
                    sgb: None,
                    sgb2: None,
                    agb: None,
                    // sgb: load(&self.custom_boot_roms.sgb),
                    // sgb2: load(&self.custom_boot_roms.sgb2),
                    // agb: load(&self.custom_boot_roms.agb),
                }
            }
        }
    }
}

impl EmulatorCore for GameBoyCore {
    type Config = GameBoyConfig;

    fn core_info() -> &'static CoreInfo {
        &CORE_INFO
    }

    fn try_from_file(data: &[u8], backup: Option<&[u8]>, config: &Self::Config) -> Result<Self>
    where
        Self: Sized,
    {
        let rom = Rom::from_bytes(data)?;
        if log_enabled!(log::Level::Info) {
            print_rom_info(&rom.info());
        }

        let gb_config = tgbr_core::Config::default()
            .set_model(config.model)
            .set_dmg_palette(config.palette.get_palette())
            .set_boot_rom(config.boot_roms());

        let gb = GameBoy::new(rom, backup.map(|r| r.to_vec()), &gb_config)?;

        let width = gb.frame_buffer().width;
        let height = gb.frame_buffer().height;

        Ok(Self {
            gb,
            config: config.clone(),
            frame_buffer: super::FrameBuffer::new(width, height),
            audio_buffer: super::AudioBuffer::default(),
        })
    }

    fn exec_frame(&mut self) {
        self.gb.exec_frame();

        let cc = make_color_correction(self.gb.model().is_cgb() && self.config.color_correction);
        cc.convert_frame_buffer(&mut self.frame_buffer, self.gb.frame_buffer());

        let audio_buffer = self.gb.audio_buffer();
        self.audio_buffer
            .samples
            .resize(self.gb.audio_buffer().buf.len(), Default::default());
        for i in 0..audio_buffer.buf.len() {
            let s = &audio_buffer.buf[i];
            self.audio_buffer.samples[i] = super::AudioSample::new(s.left, s.right);
        }
    }

    fn reset(&mut self) {
        self.gb.reset()
    }

    fn frame_buffer(&self) -> &super::FrameBuffer {
        &self.frame_buffer
    }

    fn audio_buffer(&self) -> &super::AudioBuffer {
        &self.audio_buffer
    }

    fn key_config(&self) -> &KeyConfig {
        &self.config.key_config
    }

    fn set_input(&mut self, input: &super::InputData) {
        let mut gb_input = tgbr_core::Input::default();

        let f = |key: &str| -> bool {
            input
                .inputs
                .iter()
                .find_map(|r| (r.0 == key).then(|| r.1))
                // .unwrap()
                .unwrap_or(false)
        };

        gb_input.pad.up = f("up");
        gb_input.pad.down = f("down");
        gb_input.pad.left = f("left");
        gb_input.pad.right = f("right");
        gb_input.pad.a = f("a");
        gb_input.pad.b = f("b");
        gb_input.pad.start = f("start");
        gb_input.pad.select = f("select");

        self.gb.set_input(&gb_input);
    }

    fn backup(&self) -> Option<Vec<u8>> {
        self.gb.backup_ram()
    }

    fn save_state(&self) -> Vec<u8> {
        self.gb.save_state()
    }

    fn load_state(&mut self, data: &[u8]) -> Result<()> {
        self.gb.load_state(data)
    }
}

pub fn make_color_correction(color_correction: bool) -> Box<dyn ColorCorrection> {
    if color_correction {
        Box::new(CorrectColor) as Box<dyn ColorCorrection>
    } else {
        Box::new(RawColor) as Box<dyn ColorCorrection>
    }
}

pub trait ColorCorrection {
    fn translate(&self, c: &tgbr_core::Color) -> tgbr_core::Color;

    fn convert_frame_buffer(&self, dest: &mut super::FrameBuffer, src: &tgbr_core::FrameBuffer) {
        dest.resize(src.width, src.height);

        let width = src.width;
        let height = src.height;

        for y in 0..height {
            for x in 0..width {
                let ix = y * width + x;
                let c = self.translate(&src.buf[ix]);
                dest.buffer[ix] = super::Pixel::new(c.r, c.g, c.b);
            }
        }
    }
}

struct RawColor;

impl ColorCorrection for RawColor {
    fn translate(&self, c: &tgbr_core::Color) -> tgbr_core::Color {
        *c
    }
}

struct CorrectColor;

impl ColorCorrection for CorrectColor {
    fn translate(&self, c: &tgbr_core::Color) -> tgbr_core::Color {
        let r = c.r as u16;
        let g = c.g as u16;
        let b = c.b as u16;
        tgbr_core::Color {
            r: min(240, ((r * 26 + g * 4 + b * 2) / 32) as u8),
            g: min(240, ((g * 24 + b * 8) / 32) as u8),
            b: min(240, ((r * 6 + g * 4 + b * 22) / 32) as u8),
        }
    }
}

// impl Config {
//     pub fn palette(&self) -> &PaletteSelect {
//         &self.palette
//     }

//     pub fn set_palette(&mut self, palette: PaletteSelect) {
//         self.palette = palette;
//         // self.save().unwrap();
//     }

//     pub fn key_config(&self) -> &KeyConfig {
//         &self.key_config
//     }

//     pub fn key_config_mut(&mut self) -> &mut KeyConfig {
//         &mut self.key_config
//     }

//     pub fn model(&self) -> Model {
//         self.model
//     }

//     pub fn set_model(&mut self, model: Model) {
//         self.model = model;
//         self.save().unwrap();
//     }

//     pub fn boot_rom(&self) -> &BootRom {
//         &self.boot_rom
//     }

//     pub fn set_boot_rom(&mut self, boot_rom: BootRom) {
//         self.boot_rom = boot_rom;
//         self.save().unwrap();
//     }

//     pub fn custom_boot_roms(&self) -> &CustomBootRoms {
//         &self.custom_boot_roms
//     }

//     pub fn custom_boot_roms_mut(&mut self) -> &mut CustomBootRoms {
//         &mut self.custom_boot_roms
//     }
// }

// // pub fn set_color_correction(&mut self, color_correction: bool) {
// //     self.color_correction = color_correction;
// //     self.save().unwrap();
// // }

fn extension_as_string(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str().map(|s| s.to_lowercase()))
}

fn print_rom_info(info: &[(&str, String)]) {
    use prettytable::{cell, format, row, Table};

    let mut table = Table::new();
    for (k, v) in info {
        table.add_row(row![k, v]);
    }
    table.set_titles(row!["ROM File Info"]);
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

    for line in table.to_string().lines() {
        info!("{line}");
    }
}
