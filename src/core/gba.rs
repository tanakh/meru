use meru_interface::{ConfigUi, Pixel, Ui};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tgba::{Agb, Rom};

use crate::core::{AudioBuffer, AudioSample, CoreInfo, EmulatorCore, FrameBuffer, KeyConfig};

const CORE_INFO: CoreInfo = CoreInfo {
    system_name: "Game Boy Advance (TGBA)",
    abbrev: "gba",
    file_extensions: &["gba"],
};

fn default_key_config() -> KeyConfig {
    use meru_interface::key_assign::*;

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
        controllers: vec![keys.into_iter().map(|(k, v)| (k.to_string(), v)).collect()],
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct GameBoyAdvanceConfig {
    bios: Option<PathBuf>,
}

impl ConfigUi for GameBoyAdvanceConfig {
    fn ui(&mut self, ui: &mut impl Ui) {
        ui.file("BIOS:", &mut self.bios, &[("BIOS file", &["*"])]);
        if self.bios.is_none() {
            ui.label("BIOS must be specified");
        }
    }
}

pub struct GameBoyAdvanceCore {
    agb: Agb,
    config: GameBoyAdvanceConfig,
    frame_buffer: FrameBuffer,
    audio_buffer: AudioBuffer,
}

#[derive(thiserror::Error, Debug)]
pub enum GameBoyAdvanceError {
    #[error("{0}")]
    GameBoyAdvanceError(#[from] anyhow::Error),
}

impl EmulatorCore for GameBoyAdvanceCore {
    type Config = GameBoyAdvanceConfig;
    type Error = GameBoyAdvanceError;

    fn core_info() -> &'static CoreInfo {
        &CORE_INFO
    }

    fn try_from_file(
        data: &[u8],
        backup: Option<&[u8]>,
        config: &Self::Config,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let bios = config
            .bios
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("BIOS must be specified"))?;
        let bios = fs::read(bios).map_err(anyhow::Error::from)?;

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

    fn game_info(&self) -> Vec<(String, String)> {
        self.agb.info()
    }

    fn set_config(&mut self, config: &Self::Config) {
        self.config = config.clone();
    }

    fn exec_frame(&mut self, render_graphics: bool) {
        self.agb.exec_frame(render_graphics);

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
        let mut agb_input = tgba::KeyInput::default();

        for (key, value) in &input.controllers[0] {
            match key.as_str() {
                "a" => agb_input.a = *value,
                "b" => agb_input.b = *value,
                "start" => agb_input.start = *value,
                "select" => agb_input.select = *value,
                "l" => agb_input.l = *value,
                "r" => agb_input.r = *value,
                "up" => agb_input.up = *value,
                "down" => agb_input.down = *value,
                "left" => agb_input.left = *value,
                "right" => agb_input.right = *value,
                _ => unreachable!(),
            }
        }

        self.agb.set_key_input(&agb_input);
    }

    fn backup(&self) -> Option<Vec<u8>> {
        self.agb.backup()
    }

    fn save_state(&self) -> Vec<u8> {
        self.agb.save_state()
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.agb.load_state(data)?;
        Ok(())
    }
}
