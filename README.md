# MERU

Meru is a multiple game consoles emulator written in Rust.

Current supported cores:

* [Sabicom](https://github.com/tanakh/sabicom) (NES / Famicom)
* [Super Sabicom](https://github.com/tanakh/sabicom) (SNES / Super Famicom)
* [TGBR](https://github.com/tanakh/tgbr) (Game Boy)
* [TGBA](https://github.com/tanakh/tgba) (Game Boy Advance)

## Install

### Pre-build binary

Download pre-build binary archive from [Releases Page](https://github.com/tanakh/meru/releases) and extract it to an appropriate directory.

### Build from source

First, [install the Rust toolchain](https://www.rust-lang.org/tools/install) so that you can use the `cargo` commands.

You can use the `cargo` command to build and install it from the source code.

```sh
$ cargo install meru
```

To use the development version, please clone this repository.

```sh
$ git clone https://github.com/tanakh/meru
$ cd meru
$ cargo run --release
```

On Windows, you need to install dependencies by `cargo-vcpkg`:

```sh
$ git clone https://github.com/tanakh/meru
$ cd meru
$ cargo install cargo-vcpkg # if you are not installed cargo-vcpkg yet
$ cargo vcpkg build
$ cargo build --release
```

## Usage

Execute `meru.exe` or `meru` and load ROM from GUI.

By default, the Esc key returns to the menu. The hotkeys can be changed from the hotkey settings in the menu.

## License

[MIT](LICENSE)
