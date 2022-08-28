use anyhow::Result;
use enum_iterator::Sequence;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Display,
    future::Future,
    path::{Path, PathBuf},
};

use crate::{
    core::{Emulator, EmulatorCores, EMULATOR_CORES},
    file::{create_dir_all, read, read_to_string, write},
    hotkey::HotKeys,
    input::KeyConfig,
};

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

#[cfg(not(target_arch = "wasm32"))]
mod dirs {
    use anyhow::{anyhow, Result};
    use directories::ProjectDirs;

    pub fn project_dirs() -> Result<ProjectDirs> {
        let ret = ProjectDirs::from("", "", "meru")
            .ok_or_else(|| anyhow!("Cannot find project directory"))?;
        Ok(ret)
    }
}

#[cfg(target_arch = "wasm32")]
mod dirs {
    use anyhow::{bail, Result};
    use directories::ProjectDirs;

    pub fn project_dirs() -> Result<ProjectDirs> {
        bail!("wasm does not support project directories")
    }
}

use dirs::project_dirs;

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
            warn!("Cannot get project directory. Defaults to `save` and `state`");
            (PathBuf::from("save"), PathBuf::from("state"))
        };

        create_dir_all(&save_dir).unwrap();
        create_dir_all(&state_dir).unwrap();

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

fn config_dir() -> Result<PathBuf> {
    let config_dir = if let Ok(project_dirs) = project_dirs() {
        project_dirs.config_dir().to_owned()
    } else {
        warn!("Cannot find project directory. Defaults to `config`");
        Path::new("config").to_owned()
    };
    create_dir_all(&config_dir)?;
    Ok(config_dir)
}

fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

impl Config {
    pub async fn save(&self) -> Result<()> {
        let s = serde_json::to_string_pretty(self)?;
        let path = config_path()?;
        write(&path, s).await?;
        info!("Saved config file: {:?}", path.display());
        Ok(())
    }

    pub fn core_config(&self, abbrev: &str) -> Value {
        if let Some(config) = self.core_configs.get(abbrev) {
            config.clone()
        } else {
            EmulatorCores::from_abbrev(abbrev).unwrap().default_config()
        }
    }

    pub fn set_core_config(&mut self, abbrev: &str, value: Value) {
        self.core_configs.insert(abbrev.to_owned(), value);
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

pub async fn load_config() -> Result<Config> {
    let ret = if let Ok(s) = read_to_string(config_path()?).await {
        let mut config: Config = serde_json::from_str(&s)?;

        for core in EMULATOR_CORES {
            let core_config = config.core_config(core.core_info().abbrev);
            if !core.check_config(core_config) {
                warn!(
                    "Config for {} is invalid. Initialize to default",
                    core.core_info().abbrev
                );
                config.set_core_config(core.core_info().abbrev, core.default_config());
            }
        }
        config
    } else {
        Config::default()
    };
    Ok(ret)
}

#[derive(Default, Serialize, Deserialize)]
pub struct PersistentState {
    pub recent: VecDeque<RecentFile>,
}

#[derive(Serialize, Deserialize)]
pub struct RecentFile {
    pub path: PathBuf,
    #[cfg(target_arch = "wasm32")]
    pub data: Vec<u8>,
}

impl PersistentState {
    pub fn add_recent(&mut self, recent: RecentFile) {
        self.recent.retain(|r| r.path != recent.path);
        self.recent.push_front(recent);
        while self.recent.len() > 20 {
            self.recent.pop_back();
        }
    }

    pub fn save(&self) -> impl Future<Output = Result<()>> {
        let s = bincode::serialize(self).unwrap();
        async move {
            write(persistent_state_path().unwrap(), s).await?;
            Ok::<(), anyhow::Error>(())
        }
    }
}

fn persistent_state_path() -> Result<PathBuf> {
    let config_dir = config_dir()?;
    create_dir_all(&config_dir)?;
    Ok(config_dir.join("state.json"))
}

pub async fn load_persistent_state() -> Result<PersistentState> {
    let ret = if let Ok(s) = read(persistent_state_path()?).await {
        if let Ok(ret) = bincode::deserialize(&s) {
            ret
        } else {
            Default::default()
        }
    } else {
        Default::default()
    };
    Ok(ret)
}
