use anyhow::{bail, Result};
use chrono::prelude::*;
use log::info;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn atomic_write_file(file: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut f = tempfile::NamedTempFile::new()?;
    f.write_all(data)?;
    f.persist(file)?;
    Ok(())
}

pub fn get_save_dir(core_abbrev: &str, save_dir: &Path) -> Result<PathBuf> {
    let dir = save_dir.join(core_abbrev);
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    } else if !dir.is_dir() {
        bail!("`{}` is not a directory", dir.display());
    }
    Ok(dir)
}

fn get_backup_file_path(core_abbrev: &str, name: &str, save_dir: &Path) -> Result<PathBuf> {
    Ok(get_save_dir(core_abbrev, save_dir)?.join(format!("{name}.save")))
}

fn get_state_file_path(
    core_abbrev: &str,
    name: &str,
    slot: usize,
    save_dir: &Path,
) -> Result<PathBuf> {
    Ok(get_save_dir(core_abbrev, save_dir)?.join(format!("{name}-{slot}.state")))
}

pub fn load_backup(core_abbrev: &str, name: &str, save_dir: &Path) -> Result<Option<Vec<u8>>> {
    let path = get_backup_file_path(core_abbrev, name, save_dir)?;

    Ok(if path.is_file() {
        info!("Loading backup RAM: `{}`", path.display());
        Some(std::fs::read(path)?)
    } else {
        None
    })
}

pub fn save_backup(core_abbrev: &str, name: &str, ram: &[u8], save_dir: &Path) -> Result<()> {
    let path = get_backup_file_path(core_abbrev, name, save_dir)?;

    if !path.exists() {
        info!("Creating backup RAM file: `{}`", path.display());
    } else {
        info!("Overwriting backup RAM file: `{}`", path.display());
    }
    atomic_write_file(&path, ram)
}

pub fn save_state(
    core_abbrev: &str,
    name: &str,
    slot: usize,
    data: &[u8],
    state_dir: &Path,
) -> Result<()> {
    atomic_write_file(
        &get_state_file_path(core_abbrev, name, slot, state_dir)?,
        data,
    )
}

pub fn load_state(core_abbrev: &str, name: &str, slot: usize, state_dir: &Path) -> Result<Vec<u8>> {
    let ret = fs::read(get_state_file_path(core_abbrev, name, slot, state_dir)?)?;
    Ok(ret)
}

pub fn state_date(
    core_abbrev: &str,
    name: &str,
    slot: usize,
    state_dir: &Path,
) -> Result<Option<DateTime<Local>>> {
    let path = get_state_file_path(core_abbrev, name, slot, state_dir)?;
    let metadata = fs::metadata(&path);
    if let Ok(metadata) = metadata {
        Ok(Some(metadata.modified()?.into()))
    } else {
        Ok(None)
    }
}
