use anyhow::{anyhow, Result};
use bevy::prelude::*;
use directories::ProjectDirs;
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use crate::{core::EmulatorCore, hotkey::HotKeys, key_assign::*};

// const AUDIO_FREQUENCY: usize = 48000;
// const AUDIO_BUFFER_SAMPLES: usize = 2048;

#[derive(Serialize, Deserialize)]
pub struct SystemKeyConfig {
    pub up: KeyAssign,
    pub down: KeyAssign,
    pub left: KeyAssign,
    pub right: KeyAssign,
    pub ok: KeyAssign,
    pub cancel: KeyAssign,
}

impl Default for SystemKeyConfig {
    fn default() -> Self {
        Self {
            up: any!(keycode!(Up), pad_button!(0, DPadUp)),
            down: any!(keycode!(Down), pad_button!(0, DPadDown)),
            left: any!(keycode!(Left), pad_button!(0, DPadLeft)),
            right: any!(keycode!(Right), pad_button!(0, DPadRight)),
            ok: any!(keycode!(X), pad_button!(0, South)),
            cancel: any!(keycode!(Z), pad_button!(0, West)),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    save_dir: PathBuf,
    state_dir: PathBuf,
    show_fps: bool,
    frame_skip_on_turbo: usize,
    scaling: usize,
    auto_state_save_freq: usize,
    auto_state_save_limit: usize,
    system_key_config: SystemKeyConfig,
    hotkeys: HotKeys,

    #[serde(default)]
    core_configs: BTreeMap<String, Value>,
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
            state_dir,
            show_fps: false,
            frame_skip_on_turbo: 4,
            scaling: 4,
            auto_state_save_freq: 60,
            auto_state_save_limit: 10 * 60,
            system_key_config: SystemKeyConfig::default(),
            hotkeys: HotKeys::default(),
            core_configs: BTreeMap::new(),
        }
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        self.save().unwrap();
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

    pub fn scaling(&self) -> usize {
        self.scaling
    }

    pub fn set_scaling(&mut self, scaling: usize) {
        self.scaling = scaling;
        self.save().unwrap();
    }

    pub fn show_fps(&self) -> bool {
        self.show_fps
    }

    pub fn set_show_fps(&mut self, show_fps: bool) {
        self.show_fps = show_fps;
        self.save().unwrap();
    }

    pub fn save_dir(&self) -> &Path {
        &self.save_dir
    }

    pub fn set_save_dir(&mut self, save_dir: PathBuf) {
        self.save_dir = save_dir;
        self.save().unwrap();
    }

    pub fn state_dir(&self) -> &PathBuf {
        &self.state_dir
    }

    pub fn set_state_dir(&mut self, state_dir: PathBuf) {
        self.state_dir = state_dir;
        self.save().unwrap();
    }

    pub fn system_key_config(&self) -> &SystemKeyConfig {
        &self.system_key_config
    }

    pub fn hotkeys(&self) -> &HotKeys {
        &self.hotkeys
    }

    pub fn hotkeys_mut(&mut self) -> &mut HotKeys {
        &mut self.hotkeys
    }

    pub fn auto_state_save_freq(&self) -> usize {
        self.auto_state_save_freq
    }

    pub fn auto_state_save_limit(&self) -> usize {
        self.auto_state_save_limit
    }

    pub fn frame_skip_on_turbo(&self) -> usize {
        self.frame_skip_on_turbo
    }

    pub fn set_frame_skip_on_turbo(&mut self, frame_skip_on_turbo: usize) {
        self.frame_skip_on_turbo = frame_skip_on_turbo;
        self.save().unwrap();
    }

    pub fn core_config<T: EmulatorCore>(&self) -> T::Config {
        if let Some(config) = self.core_configs.get(T::core_info().system_name) {
            serde_json::from_value(config.clone()).unwrap()
        } else {
            <T as EmulatorCore>::Config::default()
        }
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
