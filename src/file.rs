use anyhow::{anyhow, bail, Result};
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

fn get_state_file_path(rom_file: &Path, slot: usize, state_dir: &Path) -> Result<PathBuf> {
    let state_file = rom_file
        .file_stem()
        .ok_or_else(|| anyhow!("Invalid file name: {}", rom_file.display()))?;
    let state_file = format!("{}-{slot}", state_file.to_string_lossy());

    if !state_dir.exists() {
        fs::create_dir_all(state_dir)?;
    } else if !state_dir.is_dir() {
        bail!("`{}` is not a directory", state_dir.display());
    }

    // with_extension() is not correct when filename contains '.'
    // so just add extension to string
    let state_file = format!("{state_file}.state");

    Ok(state_dir.join(state_file))
}

pub fn get_backup_dir(core_abbrev: &str, save_dir: &Path) -> Result<PathBuf> {
    let dir = save_dir.join(core_abbrev);
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

pub fn load_backup(core_abbrev: &str, name: &str, save_dir: &Path) -> Result<Option<Vec<u8>>> {
    let save_file_path = get_backup_dir(core_abbrev, save_dir)?.join(format!("{name}.sav"));

    Ok(if save_file_path.is_file() {
        info!("Loading backup RAM: `{}`", save_file_path.display());
        Some(std::fs::read(save_file_path)?)
    } else {
        None
    })
}

pub fn save_backup(core_abbrev: &str, name: &str, ram: &[u8], save_dir: &Path) -> Result<()> {
    let save_file_path = get_backup_dir(core_abbrev, save_dir)?.join(format!("{name}.sav"));

    if !save_file_path.exists() {
        info!("Creating backup RAM file: `{}`", save_file_path.display());
    } else {
        info!(
            "Overwriting backup RAM file: `{}`",
            save_file_path.display()
        );
    }
    atomic_write_file(&save_file_path, ram)
}

pub fn save_state_data(rom_file: &Path, slot: usize, data: &[u8], state_dir: &Path) -> Result<()> {
    atomic_write_file(&get_state_file_path(rom_file, slot, state_dir)?, data)?;
    Ok(())
}

pub fn load_state_data(rom_file: &Path, slot: usize, state_dir: &Path) -> Result<Vec<u8>> {
    let ret = fs::read(get_state_file_path(rom_file, slot, state_dir)?)?;
    Ok(ret)
}

pub fn state_data_date(
    rom_file: &Path,
    slot: usize,
    state_dir: &Path,
) -> Result<Option<DateTime<Local>>> {
    let path = get_state_file_path(rom_file, slot, state_dir)?;
    let metadata = fs::metadata(&path);
    if let Ok(metadata) = metadata {
        Ok(Some(metadata.modified()?.into()))
    } else {
        Ok(None)
    }
}
