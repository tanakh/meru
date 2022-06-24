use anyhow::Result;
use bevy::prelude::*;
use bevy_egui::egui::{self, SelectableLabel};
use serde::{Deserialize, Serialize};
use std::{cmp::min, path::PathBuf};

use tgbr_core::{BootRoms, Color, GameBoy, Model, Rom};

use crate::{
    core::{ConfigUi, CoreInfo, EmulatorCore, KeyConfig},
    key_assign::*,
    menu::file_field,
};

const CORE_INFO: CoreInfo = CoreInfo {
    system_name: "Game Boy (TGBR)",
    abbrev: "gb",
    file_extensions: &["gb", "gbc"],
};

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

pub struct GameBoyCore {
    gb: GameBoy,
    config: GameBoyConfig,
    frame_buffer: super::FrameBuffer,
    audio_buffer: super::AudioBuffer,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GameBoyConfig {
    model: Model,
    boot_rom: BootRom,
    custom_boot_roms: CustomBootRoms,
    palette: PaletteSelect,
    color_correction: bool,
}

impl Default for GameBoyConfig {
    fn default() -> Self {
        Self {
            model: Model::Auto,
            boot_rom: BootRom::Internal,
            custom_boot_roms: CustomBootRoms::default(),
            palette: PaletteSelect::Pocket,
            color_correction: true,
        }
    }
}

impl ConfigUi for GameBoyConfig {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Model:");
            ui.radio_value(&mut self.model, Model::Auto, "Auto");
            ui.radio_value(&mut self.model, Model::Dmg, "GameBoy");
            ui.radio_value(&mut self.model, Model::Cgb, "GameBoy Color");
        });

        ui.label("Boot ROM:");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.boot_rom, BootRom::None, "Do not use");
            ui.radio_value(&mut self.boot_rom, BootRom::Internal, "Use internal ROM");
            ui.radio_value(&mut self.boot_rom, BootRom::Custom, "Use specified ROM");
        });

        ui.add_enabled_ui(self.boot_rom == BootRom::Custom, |ui| {
            file_field(
                ui,
                "DMG boot ROM:",
                &mut self.custom_boot_roms.dmg,
                &[("Boot ROM file", &["*"])],
                true,
            );
            file_field(
                ui,
                "CGB boot ROM:",
                &mut self.custom_boot_roms.cgb,
                &[("Boot ROM file", &["*"])],
                true,
            );
        });

        ui.label("Graphics:");
        ui.checkbox(&mut self.color_correction, "Color Correction");

        ui.label("GameBoy Palette:");

        ui.horizontal(|ui| {
            egui::ComboBox::from_label("")
                .width(250.0)
                .selected_text(match self.palette {
                    PaletteSelect::Dmg => "GameBoy",
                    PaletteSelect::Pocket => "GameBoy Pocket",
                    PaletteSelect::Light => "GameBoy Light",
                    PaletteSelect::Grayscale => "Grayscale",
                    PaletteSelect::Custom(_) => "Custom",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.palette, PaletteSelect::Dmg, "GameBoy");
                    ui.selectable_value(&mut self.palette, PaletteSelect::Pocket, "GameBoy Pocket");
                    ui.selectable_value(&mut self.palette, PaletteSelect::Light, "GameBoy Light");
                    ui.selectable_value(&mut self.palette, PaletteSelect::Grayscale, "Grayscale");
                    if ui
                        .add(SelectableLabel::new(
                            matches!(self.palette, PaletteSelect::Custom(_)),
                            "Custom",
                        ))
                        .clicked()
                    {
                        self.palette = PaletteSelect::Custom(*self.palette.get_palette());
                    }
                });

            let cols = *self.palette.get_palette();

            for i in (0..4).rev() {
                let mut col = [cols[i].r, cols[i].g, cols[i].b];
                ui.color_edit_button_srgb(&mut col).changed();

                if let PaletteSelect::Custom(r) = &mut self.palette {
                    r[i] = tgbr_core::Color::new(col[0], col[1], col[2]);
                }
            }
        });

        // if &pal != config.palette() {
        //     if let Some(gb_state) = gb_state.as_mut() {
        //         gb_state.gb.set_dmg_palette(pal.get_palette());
        //     }
        //     config.set_palette(pal);
        //     *last_palette_changed = Some(SystemTime::now());
        // }
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

    fn default_key_config() -> KeyConfig {
        default_key_config()
    }

    fn set_input(&mut self, input: &super::InputData) {
        let mut gb_input = tgbr_core::Input::default();

        gb_input.pad.up = input.get("up");
        gb_input.pad.down = input.get("down");
        gb_input.pad.left = input.get("left");
        gb_input.pad.right = input.get("right");
        gb_input.pad.a = input.get("a");
        gb_input.pad.b = input.get("b");
        gb_input.pad.start = input.get("start");
        gb_input.pad.select = input.get("select");

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
