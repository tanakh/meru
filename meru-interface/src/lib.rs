pub mod key_assign;

use anyhow::Result;
use bevy_egui::egui;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::key_assign::{InputState, KeyAssign};

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

#[derive(Default, Clone)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Pixel {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Default)]
pub struct AudioBuffer {
    pub samples: Vec<AudioSample>,
}

#[derive(Default, Clone)]
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
    pub keys: Vec<(String, KeyAssign)>,
}

impl KeyConfig {
    pub fn input(&self, input_state: &InputState) -> InputData {
        let mut inputs = Vec::with_capacity(self.keys.len());

        for (key, assign) in &self.keys {
            inputs.push((key.clone(), assign.pressed(input_state)));
        }

        InputData { inputs }
    }
}

#[derive(Default)]
pub struct InputData {
    pub inputs: Vec<(String, bool)>,
}

impl InputData {
    pub fn get(&self, key: &str) -> bool {
        self.inputs
            .iter()
            .find_map(|(k, v)| (k == key).then(|| *v))
            .unwrap()
    }
}

pub trait ConfigUi {
    fn ui(&mut self, ui: &mut egui::Ui);
}

pub trait EmulatorCore {
    type Config: ConfigUi + Serialize + DeserializeOwned + Default;

    fn core_info() -> &'static CoreInfo;

    fn try_from_file(data: &[u8], backup: Option<&[u8]>, config: &Self::Config) -> Result<Self>
    where
        Self: Sized;
    fn game_info(&self) -> Vec<(String, String)>;

    fn set_config(&mut self, config: &Self::Config);

    fn exec_frame(&mut self);
    fn reset(&mut self);

    fn frame_buffer(&self) -> &FrameBuffer;
    fn audio_buffer(&self) -> &AudioBuffer;

    fn default_key_config() -> KeyConfig;
    fn set_input(&mut self, input: &InputData);

    fn backup(&self) -> Option<Vec<u8>>;

    fn save_state(&self) -> Vec<u8>;
    fn load_state(&mut self, data: &[u8]) -> Result<()>;
}
