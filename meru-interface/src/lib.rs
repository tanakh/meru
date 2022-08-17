pub mod key_assign;

use std::path::PathBuf;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub use crate::key_assign::{
    Gamepad, GamepadAxis, GamepadAxisType, GamepadButton, GamepadButtonType, InputState, KeyAssign,
    KeyCode, MultiKey, SingleKey,
};

pub struct CoreInfo {
    pub system_name: &'static str,
    pub abbrev: &'static str,
    pub file_extensions: &'static [&'static str],
}

#[derive(Default)]
pub struct FrameBuffer {
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<Pixel>,
}

impl FrameBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let mut ret = Self::default();
        ret.resize(width, height);
        ret
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.buffer.resize(width * height, Pixel::default());
    }

    pub fn pixel(&self, x: usize, y: usize) -> &Pixel {
        &self.buffer[y * self.width + x]
    }

    pub fn pixel_mut(&mut self, x: usize, y: usize) -> &mut Pixel {
        &mut self.buffer[y * self.width + x]
    }
}

#[derive(Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Pixel {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

pub struct AudioBuffer {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<AudioSample>,
}

impl Default for AudioBuffer {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            samples: vec![],
        }
    }
}

#[derive(Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioSample {
    pub left: i16,
    pub right: i16,
}

impl AudioSample {
    pub fn new(left: i16, right: i16) -> Self {
        Self { left, right }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct KeyConfig {
    pub controllers: Vec<Vec<(String, KeyAssign)>>,
}

impl KeyConfig {
    pub fn input(&self, input_state: &impl InputState) -> InputData {
        let controllers = self
            .controllers
            .iter()
            .map(|keys| {
                keys.iter()
                    .map(|(key, assign)| (key.clone(), assign.pressed(input_state)))
                    .collect()
            })
            .collect();

        InputData { controllers }
    }
}

#[derive(Default)]
pub struct InputData {
    pub controllers: Vec<Vec<(String, bool)>>,
}

pub trait ConfigUi {
    fn ui<'a, 'b>(&'a mut self, ui: &'b mut impl Ui);
}

pub trait Ui {
    fn horizontal(&mut self, f: impl FnOnce(&mut Self));
    fn enabled(&mut self, enabled: bool, f: impl FnOnce(&mut Self));
    fn label(&mut self, text: &str);
    fn checkbox(&mut self, value: &mut bool, text: &str);
    fn file(&mut self, label: &str, value: &mut Option<PathBuf>, filter: &[(&str, &[&str])]);
    fn color(&mut self, value: &mut Pixel);
    fn radio<T: PartialEq + Clone>(&mut self, value: &mut T, choices: &[(&str, T)]);
    fn combo_box<T: PartialEq + Clone>(&mut self, value: &mut T, choices: &[(&str, T)]);
}

pub trait EmulatorCore {
    type Error: std::error::Error + Send + Sync + 'static;
    type Config: ConfigUi + Serialize + DeserializeOwned + Default;

    fn core_info() -> &'static CoreInfo;

    fn try_from_file(
        data: &[u8],
        backup: Option<&[u8]>,
        config: &Self::Config,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized;
    fn game_info(&self) -> Vec<(String, String)>;

    fn set_config(&mut self, config: &Self::Config);

    fn exec_frame(&mut self, render_graphics: bool);
    fn reset(&mut self);

    fn frame_buffer(&self) -> &FrameBuffer;
    fn audio_buffer(&self) -> &AudioBuffer;

    fn default_key_config() -> KeyConfig;
    fn set_input(&mut self, input: &InputData);

    fn backup(&self) -> Option<Vec<u8>>;

    fn save_state(&self) -> Vec<u8>;
    fn load_state(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}
