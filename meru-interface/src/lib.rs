#[cfg(target_arch = "wasm32")]
#[macro_use]
extern crate base64_serde;

pub mod config;
pub mod key_assign;

pub use config::File;

use schemars::{
    gen::SchemaGenerator,
    schema::{Schema, SchemaObject},
    JsonSchema,
};
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
    pub buffer: Vec<Color>,
}

impl FrameBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let mut ret = Self::default();
        ret.resize(width, height);
        ret
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        if (width, height) == (self.width, self.height) {
            return;
        }
        self.width = width;
        self.height = height;
        self.buffer.resize(width * height, Color::default());
    }

    pub fn pixel(&self, x: usize, y: usize) -> &Color {
        &self.buffer[y * self.width + x]
    }

    pub fn pixel_mut(&mut self, x: usize, y: usize) -> &mut Color {
        &mut self.buffer[y * self.width + x]
    }
}

#[derive(Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl JsonSchema for Color {
    fn schema_name() -> String {
        "Color".to_string()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        let mut schema: SchemaObject = <String>::json_schema(gen).into();
        schema.format = Some("color".to_owned());
        schema.into()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ParseColorError {
    #[error("Color string must be hex color code: `#RRGGBB`")]
    InvalidFormat,
}

impl TryFrom<String> for Color {
    type Error = ParseColorError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.len() != 7 || &s[0..1] != "#" || !s[1..].chars().all(|c| c.is_ascii_hexdigit()) {
            Err(ParseColorError::InvalidFormat)?;
        }

        Ok(Color {
            r: u8::from_str_radix(&s[1..3], 16).unwrap(),
            g: u8::from_str_radix(&s[3..5], 16).unwrap(),
            b: u8::from_str_radix(&s[5..7], 16).unwrap(),
        })
    }
}

impl From<Color> for String {
    fn from(c: Color) -> Self {
        format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
    }
}

impl Color {
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

impl AudioBuffer {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
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

pub trait EmulatorCore {
    type Error: std::error::Error + Send + Sync + 'static;
    type Config: JsonSchema + Serialize + DeserializeOwned + Default;

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
