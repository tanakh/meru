use anyhow::{anyhow, bail, Result};
use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use bevy_tiled_camera::{TiledCamera, TiledCameraBundle};
use meru_interface::{
    AudioBuffer, ConfigUi, CoreInfo, EmulatorCore, FrameBuffer, InputData, KeyConfig,
};
use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{Seek, SeekFrom},
    marker::PhantomData,
    path::{Path, PathBuf},
};

use crate::{
    app::{AppState, ScreenSprite, WindowControlEvent},
    config::Config,
    file::{load_backup, load_state, save_backup, save_state},
    hotkey,
    input::InputState,
    menu::EguiUi,
    rewinding::AutoSavedState,
};

macro_rules! def_emulator_cores {
    ($( $constr:ident($t:ty) ),* $(,)?) => {
        pub enum EmulatorCores {
            $(
                $constr(PhantomData<$t>),
            )*
        }

        const EMULATOR_CORES: &[EmulatorCores] = &[
            $(
                EmulatorCores::$constr(PhantomData),
            )*
        ];

        macro_rules! dispatch_enum {
            ($enum:ident, $core:ident, $var:ident, $e:expr) => {
                match $core {
                    $(
                        $enum::$constr($var) => $e,
                    )*
                }
            };
        }

        pub enum EmulatorEnum {
            $(
                $constr(Box<$t>),
            )*
        }

        $(
            impl From<$t> for EmulatorEnum {
                fn from(core: $t) -> Self {
                    EmulatorEnum::$constr(Box::new(core))
                }
            }
        )*
    };
}

def_emulator_cores!(
    GameBoy(tgbr::GameBoy),
    GameBoyAdvance(tgba::Agb),
    Snes(super_sabicom::Snes),
);

impl EmulatorCores {
    fn core_info(&self) -> &CoreInfo {
        fn core_info<T: EmulatorCore>(_: &PhantomData<T>) -> &'static CoreInfo {
            T::core_info()
        }
        dispatch_enum!(EmulatorCores, self, core, core_info(core))
    }
}

fn make_core_from_data<T: EmulatorCore + Into<EmulatorEnum>, F: FnMut() -> Result<Vec<u8>>>(
    _: &PhantomData<T>,
    name: &str,
    ext: &str,
    mut data: F,
    config: &Config,
) -> Option<Result<EmulatorEnum>> {
    let core_info = <T as EmulatorCore>::core_info();
    if !core_info.file_extensions.contains(&ext) {
        None?;
    }

    let mut f = || {
        let backup = load_backup(core_info.abbrev, name, &config.save_dir)?;
        let data = data()?;
        let core = T::try_from_file(&data, backup.as_deref(), &config.core_config::<T>())?;
        Ok(core.into())
    };
    Some(f())
}

impl EmulatorEnum {
    pub fn try_new(
        name: &str,
        ext: &str,
        mut data: impl FnMut() -> Result<Vec<u8>>,
        config: &Config,
    ) -> Result<Self> {
        for core in EMULATOR_CORES {
            if let Some(ret) = dispatch_enum!(
                EmulatorCores,
                core,
                core,
                make_core_from_data(core, name, ext, &mut data, config)
            ) {
                return ret;
            }
        }
        bail!("No supported core");
    }

    pub fn core_info(&self) -> &CoreInfo {
        fn core_info<T: EmulatorCore>(_: &T) -> &'static CoreInfo {
            T::core_info()
        }
        dispatch_enum!(EmulatorEnum, self, core, core_info(core.as_ref()))
    }

    pub fn game_info(&self) -> Vec<(String, String)> {
        dispatch_enum!(EmulatorEnum, self, core, core.game_info())
    }

    pub fn backup(&self) -> Option<Vec<u8>> {
        dispatch_enum!(EmulatorEnum, self, core, core.backup())
    }

    pub fn set_config(&mut self, config: &Config) {
        fn set_config<T: EmulatorCore>(core: &mut T, config: &Config) {
            core.set_config(&config.core_config::<T>());
        }
        dispatch_enum!(EmulatorEnum, self, core, set_config(core.as_mut(), config));
    }

    pub fn reset(&mut self) {
        dispatch_enum!(EmulatorEnum, self, core, core.reset());
    }

    pub fn exec_frame(&mut self, render_graphics: bool) {
        dispatch_enum!(EmulatorEnum, self, core, core.exec_frame(render_graphics));
    }

    pub fn frame_buffer(&self) -> &FrameBuffer {
        dispatch_enum!(EmulatorEnum, self, core, core.frame_buffer())
    }

    pub fn audio_buffer(&self) -> &AudioBuffer {
        dispatch_enum!(EmulatorEnum, self, core, core.audio_buffer())
    }

    pub fn set_input(&mut self, input: &InputData) {
        dispatch_enum!(EmulatorEnum, self, core, core.set_input(input));
    }

    pub fn save_state(&self) -> Vec<u8> {
        dispatch_enum!(EmulatorEnum, self, core, core.save_state())
    }

    pub fn load_state(&mut self, data: &[u8]) -> Result<()> {
        dispatch_enum!(EmulatorEnum, self, core, core.load_state(data)?);
        Ok(())
    }
}

pub struct Emulator {
    pub core: EmulatorEnum,
    pub game_name: String,
    pub auto_saved_states: VecDeque<AutoSavedState>,
    total_auto_saved_size: usize,
    prev_auto_saved_frame: usize,
    prev_backup_saved_frame: usize,
    save_dir: PathBuf,
    frames: usize,
}

impl Drop for Emulator {
    fn drop(&mut self) {
        if let Some(ram) = self.core.backup() {
            if let Err(err) = save_backup(
                self.core.core_info().abbrev,
                &self.game_name,
                &ram,
                &self.save_dir,
            ) {
                error!("Failed to save backup ram: {err}");
            }
        } else {
            info!("No backup RAM to save");
        }
    }
}

pub const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "7z", "rar"];

fn is_archive_file(path: &Path) -> bool {
    path.extension().map_or(false, |ext| {
        let ext = ext.to_string_lossy();
        ARCHIVE_EXTENSIONS.iter().any(|r| *r == ext.as_ref())
    })
}

fn try_make_emulator(
    path: &Path,
    mut data: impl FnMut() -> Result<Vec<u8>>,
    config: &Config,
) -> Result<Emulator> {
    let ext = path
        .extension()
        .ok_or_else(|| anyhow!("Cannot detect file type"))?
        .to_string_lossy();

    let name = path
        .file_stem()
        .ok_or_else(|| anyhow!("Invalid file name"))?
        .to_string_lossy();

    let core = EmulatorEnum::try_new(&name, &ext, &mut data, config)?;

    Ok(Emulator {
        core,
        game_name: name.to_string(),
        auto_saved_states: VecDeque::new(),
        total_auto_saved_size: 0,
        prev_auto_saved_frame: 0,
        prev_backup_saved_frame: 0,
        save_dir: config.save_dir.clone(),
        frames: 0,
    })
}

fn config_ui<T: EmulatorCore>(_: &PhantomData<T>, ui: &mut EguiUi, config: &mut Config) {
    let mut core_config = config.core_config::<T>();
    core_config.ui(ui);
    config.set_core_config::<T>(core_config);
}

impl Emulator {
    pub fn core_infos() -> Vec<&'static CoreInfo> {
        let mut ret = vec![];
        for core in EMULATOR_CORES.iter() {
            ret.push(core.core_info());
        }
        ret
    }

    pub fn config_ui(ui: &mut EguiUi, abbrev: &str, config: &mut Config) {
        for core in EMULATOR_CORES.iter() {
            if core.core_info().abbrev == abbrev {
                dispatch_enum!(EmulatorCores, core, core, config_ui(core, ui, config));
            }
        }
    }

    pub fn default_key_config(abbrev: &str) -> KeyConfig {
        fn default_key_config<T: EmulatorCore>(_: &PhantomData<T>) -> KeyConfig {
            T::default_key_config()
        }
        for core in EMULATOR_CORES.iter() {
            if core.core_info().abbrev == abbrev {
                return dispatch_enum!(EmulatorCores, core, core, default_key_config(core));
            }
        }
        panic!();
    }

    pub fn try_new(path: &Path, config: &Config) -> Result<Self> {
        if is_archive_file(path) {
            let mut f = File::open(path)?;

            let files = compress_tools::list_archive_files(&mut f)?;

            for path in files {
                let res = try_make_emulator(
                    Path::new(&path),
                    || {
                        let mut data = vec![];
                        f.seek(SeekFrom::Start(0))?;
                        compress_tools::uncompress_archive_file(&mut f, &mut data, &path)?;
                        Ok(data)
                    },
                    config,
                );
                if res.is_ok() {
                    return res;
                }
            }

            bail!("File does not contain a supported file");
        } else {
            try_make_emulator(
                path,
                || {
                    let data = fs::read(path)?;
                    Ok(data)
                },
                config,
            )
        }
    }

    pub fn reset(&mut self) {
        self.core.reset();
    }

    pub fn save_backup(&mut self) -> Result<()> {
        if let Some(ram) = self.core.backup() {
            save_backup(
                self.core.core_info().abbrev,
                &self.game_name,
                &ram,
                &self.save_dir,
            )?;
        }

        self.prev_backup_saved_frame = self.frames;
        Ok(())
    }

    pub fn push_auto_save(&mut self) {
        let saved_state = AutoSavedState {
            data: self.core.save_state(),
            thumbnail: frame_buffer_to_image(self.core.frame_buffer()),
        };
        self.auto_saved_states.push_back(saved_state);
    }

    pub fn save_state_slot(&self, slot: usize, config: &Config) -> Result<()> {
        let data = self.core.save_state();
        save_state(
            self.core.core_info().abbrev,
            &self.game_name,
            slot,
            &data,
            &config.save_dir,
        )
    }

    pub fn load_state_slot(&mut self, slot: usize, config: &Config) -> Result<()> {
        let data = load_state(
            self.core.core_info().abbrev,
            &self.game_name,
            slot,
            &config.save_dir,
        )?;
        self.core.load_state(&data)
    }
}

pub struct EmulatorPlugin;

impl Plugin for EmulatorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputData>()
            .add_system_set(
                SystemSet::on_update(AppState::Running)
                    .with_system(emulator_input_system.label("input")),
            )
            .add_system_set(
                SystemSet::on_enter(AppState::Running).with_system(setup_emulator_system),
            )
            .add_system_set(
                SystemSet::on_resume(AppState::Running).with_system(resume_emulator_system),
            )
            .add_system_set(
                SystemSet::on_update(AppState::Running)
                    .with_system(emulator_system)
                    .after("input"),
            )
            .add_system_set(
                SystemSet::on_exit(AppState::Running).with_system(exit_emulator_system),
            );
    }
}

pub fn emulator_input_system(
    mut config: ResMut<Config>,
    emulator: Res<Emulator>,
    input_keycode: Res<Input<KeyCode>>,
    input_gamepad_button: Res<Input<GamepadButton>>,
    input_gamepad_axis: Res<Axis<GamepadAxis>>,
    mut input: ResMut<InputData>,
) {
    *input = config
        .key_config(emulator.core.core_info().abbrev)
        .input(&InputState::new(
            &input_keycode,
            &input_gamepad_button,
            &input_gamepad_axis,
        ));
}

pub struct GameScreen(pub Handle<Image>);

fn setup_emulator_system(
    mut windows: ResMut<Windows>,
    mut commands: Commands,
    emulator: Res<Emulator>,
    mut images: ResMut<Assets<Image>>,
    mut event: EventWriter<WindowControlEvent>,
) {
    let width = emulator.core.frame_buffer().width.max(1) as u32;
    let height = emulator.core.frame_buffer().height.max(1) as u32;
    let img = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0; (width * height * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
    );

    let texture = images.add(img);
    commands
        .spawn_bundle(SpriteBundle {
            texture: texture.clone(),
            ..Default::default()
        })
        .insert(ScreenSprite);

    commands.insert_resource(GameScreen(texture));

    let window = windows.get_primary_mut().unwrap();
    window.set_cursor_lock_mode(true);
    window.set_cursor_visibility(false);

    event.send(WindowControlEvent::Restore);
}

fn resume_emulator_system(
    mut windows: ResMut<Windows>,
    mut event: EventWriter<WindowControlEvent>,
) {
    let window = windows.get_primary_mut().unwrap();
    window.set_cursor_lock_mode(true);
    window.set_cursor_visibility(false);

    event.send(WindowControlEvent::Restore);
}

fn exit_emulator_system(
    mut windows: ResMut<Windows>,
    mut commands: Commands,
    screen_entity: Query<Entity, With<ScreenSprite>>,
) {
    let window = windows.get_primary_mut().unwrap();
    window.set_cursor_lock_mode(false);
    window.set_cursor_visibility(true);

    commands.entity(screen_entity.single()).despawn();
}

struct AudioSource {
    sample_rate: u32,
    channels: u16,
    data: Vec<i16>,
    cursor: usize,
}

impl Iterator for AudioSource {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.data.len() {
            return None;
        }
        let sample = self.data[self.cursor];
        self.cursor += 1;
        Some(sample as i16)
    }
}

impl rodio::Source for AudioSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn emulator_system(
    mut commands: Commands,
    screen: Res<GameScreen>,
    camera: Query<(Entity, &TiledCamera)>,
    config: Res<Config>,
    mut emulator: ResMut<Emulator>,
    mut images: ResMut<Assets<Image>>,
    input: Res<InputData>,
    audio_sink: ResMut<rodio::Sink>,
    is_turbo: Res<hotkey::IsTurbo>,
) {
    emulator.core.set_input(&*input);

    let push_audio_queue = |audio_buffer: &AudioBuffer| {
        let source = AudioSource {
            sample_rate: audio_buffer.sample_rate,
            channels: audio_buffer.channels,
            data: audio_buffer
                .samples
                .iter()
                .flat_map(|sample| [sample.left, sample.right])
                .collect(),
            cursor: 0,
        };
        audio_sink.append(source);
    };

    if !is_turbo.0 {
        if audio_sink.len() as u32 > 4 {
            // execution too fast. wait 1 frame.
            return;
        }

        let mut exec_frame = |render_graphics| {
            emulator.core.exec_frame(render_graphics);
            emulator.frames += 1;

            // FIXME
            let elapsed = emulator.frames as f64 / 60.0;
            let need_more = emulator.total_auto_saved_size
                < (elapsed * config.auto_state_save_rate as f64).floor() as usize;
            let enough_span =
                emulator.prev_auto_saved_frame + config.minimum_auto_save_span < emulator.frames;

            if need_more && enough_span {
                let saved_state = AutoSavedState {
                    data: emulator.core.save_state(),
                    thumbnail: frame_buffer_to_image(emulator.core.frame_buffer()),
                };

                let state_size = saved_state.size();
                emulator.total_auto_saved_size += state_size;
                emulator.prev_auto_saved_frame = emulator.frames;

                emulator.auto_saved_states.push_back(saved_state);
                if emulator.auto_saved_states.len() * state_size > config.auto_state_save_limit {
                    emulator.auto_saved_states.pop_front();
                }
            }
            push_audio_queue(emulator.core.audio_buffer());
        };

        if audio_sink.len() < 2 {
            // execution too slow. run 2 frame for supply enough audio samples.
            exec_frame(false);
        }
        exec_frame(true);

        // Update texture
        let fb = emulator.core.frame_buffer();
        let image = images.get_mut(&screen.0).unwrap();
        copy_frame_buffer(image, fb);
    } else {
        for i in 0..config.frame_skip_on_turbo {
            emulator.core.exec_frame(i == 0);
            if audio_sink.len() < 2 {
                push_audio_queue(emulator.core.audio_buffer());
            }
        }
        // Update texture
        let fb = emulator.core.frame_buffer();
        let image = images.get_mut(&screen.0).unwrap();
        copy_frame_buffer(image, fb);
        emulator.frames += 1;
    }

    {
        let camera = camera.single();
        let image = images.get(&screen.0).unwrap();
        let image_size = image.size();
        let width = image_size[0] as u32;
        let height = image_size[1] as u32;

        if (camera.1.tile_count.x, camera.1.tile_count.y) != (width, height) {
            commands.entity(camera.0).despawn();
            commands.spawn_bundle(
                TiledCameraBundle::pixel_cam([width, height]).with_pixels_per_tile([1, 1]),
            );
        }
    }

    if emulator.prev_backup_saved_frame + 60 * 60 <= emulator.frames {
        emulator.save_backup().unwrap();
    }
}

fn frame_buffer_to_image(frame_buffer: &FrameBuffer) -> Image {
    let width = frame_buffer.width;
    let height = frame_buffer.height;

    let mut image = Image::new_fill(
        Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
    );
    copy_frame_buffer(&mut image, frame_buffer);
    image
}

fn copy_frame_buffer(image: &mut Image, frame_buffer: &FrameBuffer) {
    if frame_buffer.width == 0 || frame_buffer.height == 0 {
        return;
    }

    let width = frame_buffer.width;
    let height = frame_buffer.height;

    let image_size = image.size();
    if (image_size[0] as usize, image_size[1] as usize) != (width, height) {
        image.resize(Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        });
    }

    let data = &mut image.data;

    for y in 0..height {
        for x in 0..width {
            let ix = y * width + x;
            let pixel = &mut data[ix * 4..ix * 4 + 4];
            let c = &frame_buffer.buffer[ix];
            pixel[0] = c.r;
            pixel[1] = c.g;
            pixel[2] = c.b;
            pixel[3] = 0xff;
        }
    }
}
