use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use enum_iterator::Sequence;
use log::info;
use meru_interface::EmulatorCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Display,
    fs,
    path::{Path, PathBuf},
};

use crate::{core::Emulator, hotkey::HotKeys, input::KeyConfig};

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, Sequence)]
pub enum SystemKey {
    Up,
    Down,
    Left,
    Right,
    Ok,
    Cancel,
}

impl Display for SystemKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SystemKey::Up => "Up",
            SystemKey::Down => "Down",
            SystemKey::Left => "Left",
            SystemKey::Right => "Right",
            SystemKey::Ok => "Ok",
            SystemKey::Cancel => "Cancel",
        };
        write!(f, "{s}")
    }
}

pub type SystemKeys = KeyConfig<SystemKey>;

impl Default for SystemKeys {
    fn default() -> Self {
        use meru_interface::key_assign::*;
        use SystemKey::*;
        Self(vec![
            (Up, any!(keycode!(Up), pad_button!(0, DPadUp))),
            (Down, any!(keycode!(Down), pad_button!(0, DPadDown))),
            (Left, any!(keycode!(Left), pad_button!(0, DPadLeft))),
            (Right, any!(keycode!(Right), pad_button!(0, DPadRight))),
            (Ok, any!(keycode!(Return), pad_button!(0, East))),
            (Cancel, any!(keycode!(Back), pad_button!(0, South))),
        ])
    }
}

#[derive(PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Config {
    pub save_dir: PathBuf,
    pub show_fps: bool,
    pub frame_skip_on_turbo: usize,
    pub scaling: usize,
    pub auto_state_save_rate: usize,   // byte/s
    pub auto_state_save_limit: usize,  // byte
    pub minimum_auto_save_span: usize, // frames
    pub hotkeys: HotKeys,
    pub system_keys: SystemKeys,

    #[serde(default)]
    core_configs: BTreeMap<String, Value>,
    #[serde(default)]
    key_configs: BTreeMap<String, meru_interface::KeyConfig>,
}

impl Default for Config {
    fn default() -> Self {
        let (save_dir, state_dir) = if let Ok(project_dirs) = project_dirs() {
            (
                project_dirs.data_dir().to_owned(),
                project_dirs
                    .state_dir()
                    .unwrap_or_else(|| project_dirs.data_dir())
                    .to_owned(),
            )
        } else {
            (PathBuf::from("save"), PathBuf::from("state"))
        };

        fs::create_dir_all(&save_dir).unwrap();
        fs::create_dir_all(&state_dir).unwrap();

        Self {
            save_dir,
            show_fps: false,
            frame_skip_on_turbo: 4,
            scaling: 2,
            auto_state_save_rate: 128 * 1024,          // 128KB/s
            auto_state_save_limit: 1024 * 1024 * 1024, // 1GB
            minimum_auto_save_span: 60,
            system_keys: SystemKeys::default(),
            hotkeys: HotKeys::default(),
            core_configs: BTreeMap::new(),
            key_configs: BTreeMap::new(),
        }
    }
}

impl Config {
    pub fn save(&self) -> Result<()> {
        let s = serde_json::to_string_pretty(self)?;
        let path = config_path()?;
        fs::write(&path, s)?;
        info!("Saved config file: {:?}", path.display());
        Ok(())
    }

    pub fn core_config<T: EmulatorCore>(&self) -> T::Config {
        if let Some(config) = self.core_configs.get(T::core_info().abbrev) {
            serde_json::from_value(config.clone()).unwrap()
        } else {
            <T as EmulatorCore>::Config::default()
        }
    }

    pub fn set_core_config<T: EmulatorCore>(&mut self, config: T::Config) {
        self.core_configs.insert(
            T::core_info().abbrev.into(),
            serde_json::to_value(config).unwrap(),
        );
    }

    pub fn key_config(&mut self, abbrev: &str) -> &meru_interface::KeyConfig {
        self.key_configs
            .entry(abbrev.to_string())
            .or_insert_with(|| Emulator::default_key_config(abbrev))
    }

    pub fn set_key_config(&mut self, abbrev: &str, key_config: meru_interface::KeyConfig) {
        self.key_configs.insert(abbrev.to_string(), key_config);
    }
}

fn project_dirs() -> Result<ProjectDirs> {
    let ret = ProjectDirs::from("", "", "meru")
        .ok_or_else(|| anyhow!("Cannot find project directory"))?;
    Ok(ret)
}

fn config_path() -> Result<PathBuf> {
    let project_dirs = project_dirs()?;
    let config_dir = project_dirs.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("config.json"))
}

pub fn load_config() -> Result<Config> {
    let ret = if let Ok(s) = std::fs::read_to_string(config_path()?) {
        serde_json::from_str(&s).map_err(|e| anyhow!("{}", e))?
    } else {
        Config::default()
    };
    Ok(ret)
}

#[derive(Default, Serialize, Deserialize)]
pub struct PersistentState {
    pub recent: VecDeque<PathBuf>,
}

impl Drop for PersistentState {
    fn drop(&mut self) {
        let s = serde_json::to_string_pretty(self).unwrap();
        fs::write(persistent_state_path().unwrap(), s).unwrap();
    }
}

impl PersistentState {
    pub fn add_recent(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_owned();
        if self.recent.contains(&path) {
            self.recent.retain(|p| p != &path);
        }
        self.recent.push_front(path);
        while self.recent.len() > 20 {
            self.recent.pop_back();
        }
    }
}

fn persistent_state_path() -> Result<PathBuf> {
    let project_dirs = project_dirs()?;
    let config_dir = project_dirs.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("state.json"))
}

pub fn load_persistent_state() -> Result<PersistentState> {
    let ret = if let Ok(s) = std::fs::read_to_string(persistent_state_path()?) {
        serde_json::from_str(&s).map_err(|e| anyhow!("{}", e))?
    } else {
        Default::default()
    };
    Ok(ret)
}
