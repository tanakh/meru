use anyhow::{anyhow, Result};
use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use tgba_core::{Agb, Rom};

use crate::{
    core::{
        AudioBuffer, AudioSample, ConfigUi, CoreInfo, EmulatorCore, FrameBuffer, KeyConfig, Pixel,
    },
    key_assign::*,
    menu::file_field,
};

const CORE_INFO: CoreInfo = CoreInfo {
    system_name: "Game Boy Advance (TGBA)",
    abbrev: "gba",
    file_extensions: &["gba"],
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
        ("l", any!(keycode!(A), pad_button!(0, LeftTrigger))),
        ("r", any!(keycode!(S), pad_button!(0, RightTrigger))),
        ("start", any!(keycode!(Return), pad_button!(0, Start))),
        ("select", any!(keycode!(RShift), pad_button!(0, Select))),
    ];

    KeyConfig {
        keys: keys.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    }
}

impl Default for KeyConfig {
    fn default() -> Self {
        default_key_config()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GameBoyAdvanceConfig {
    bios: Option<PathBuf>,
}

impl Default for GameBoyAdvanceConfig {
    fn default() -> Self {
        Self { bios: None }
    }
}

impl ConfigUi for GameBoyAdvanceConfig {
    fn ui(&mut self, ui: &mut egui::Ui) {
        file_field(ui, "BIOS:", &mut self.bios, &[("BIOS file", &["*"])], true);
        if self.bios.is_none() {
            ui.colored_label(egui::Color32::RED, "BIOS must be specified");
        }
    }
}

pub struct GameBoyAdvanceCore {
    agb: Agb,
    config: GameBoyAdvanceConfig,
    frame_buffer: FrameBuffer,
    audio_buffer: AudioBuffer,
}

impl EmulatorCore for GameBoyAdvanceCore {
    type Config = GameBoyAdvanceConfig;

    fn core_info() -> &'static CoreInfo {
        &CORE_INFO
    }

    fn try_from_file(data: &[u8], backup: Option<&[u8]>, config: &Self::Config) -> Result<Self>
    where
        Self: Sized,
    {
        let bios = config
            .bios
            .as_ref()
            .ok_or_else(|| anyhow!("BIOS must be specified"))?;
        let bios = fs::read(bios)?;

        let rom = Rom::from_bytes(data)?;

        let agb = Agb::new(bios, rom, backup.map(|r| r.to_owned()));

        let width = agb.frame_buf().width();
        let height = agb.frame_buf().height();

        Ok(Self {
            agb,
            config: config.clone(),
            frame_buffer: FrameBuffer::new(width as _, height as _),
            audio_buffer: Default::default(),
        })
    }

    fn exec_frame(&mut self) {
        self.agb.exec_frame();

        let fb = self.agb.frame_buf();
        self.frame_buffer.resize(fb.width() as _, fb.height() as _);
        for y in 0..fb.height() {
            for x in 0..fb.width() {
                let c = fb.pixel(x, y);
                *self.frame_buffer.pixel_mut(x as usize, y as usize) = Pixel::new(c.r, c.g, c.b);
            }
        }

        let ab = self.agb.audio_buf();
        self.audio_buffer
            .samples
            .resize(ab.len(), Default::default());

        for i in 0..ab.len() {
            let s = &ab.buf[i];
            self.audio_buffer.samples[i] = AudioSample::new(s.left, s.right);
        }
    }

    fn reset(&mut self) {
        self.agb.reset();
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
        let agb_input = tgba_core::KeyInput {
            a: input.get("a"),
            b: input.get("b"),
            start: input.get("start"),
            select: input.get("select"),
            l: input.get("l"),
            r: input.get("r"),
            up: input.get("up"),
            down: input.get("down"),
            left: input.get("left"),
            right: input.get("right"),
        };
        self.agb.set_key_input(&agb_input);
    }

    fn backup(&self) -> Option<Vec<u8>> {
        self.agb.backup()
    }

    fn save_state(&self) -> Vec<u8> {
        self.agb.save_state()
    }

    fn load_state(&mut self, data: &[u8]) -> Result<()> {
        self.agb.load_state(data)
    }
}
