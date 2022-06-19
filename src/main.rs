#![windows_subsystem = "windows"]

use anyhow::Result;
use std::path::PathBuf;

#[argopt::cmd]
fn main(
    /// Path to Cartridge ROM
    rom_file: Option<PathBuf>,
) -> Result<()> {
    meru::app::main(rom_file)
}
