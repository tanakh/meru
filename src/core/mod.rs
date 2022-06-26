pub mod gb;
pub mod gba;

use anyhow::{anyhow, bail, Result};
use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use bevy_egui::egui;
use bevy_tiled_camera::TiledCameraBundle;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{Seek, SeekFrom},
    path::{Path, PathBuf},
};

use crate::{
    app::{AppState, AudioStreamQueue, ScreenSprite, TiledCamera, WindowControlEvent},
    config::Config,
    file::{load_backup, load_state, save_backup, save_state},
    hotkey,
    key_assign::*,
    rewinding::AutoSavedState,
};

#[derive(Default)]
pub struct FrameBuffer {
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<Pixel>,
}

impl FrameBuffer {
    fn new(width: usize, height: usize) -> Self {
        let mut ret = Self::default();
        ret.resize(width, height);
        ret
    }

    fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.buffer.resize(width * height, Pixel::default());
    }

    fn pixel(&self, x: usize, y: usize) -> &Pixel {
        &self.buffer[y * self.width + x]
    }

    fn pixel_mut(&mut self, x: usize, y: usize) -> &mut Pixel {
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

pub trait ConfigUi {
    fn ui(&mut self, ui: &mut egui::Ui);
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct KeyConfig {
    pub keys: Vec<(String, KeyAssign)>,
}

impl KeyConfig {
    fn input(&self, input_state: &InputState) -> InputData {
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
    fn get(&self, key: &str) -> bool {
        self.inputs
            .iter()
            .find_map(|(k, v)| (k == key).then(|| *v))
            .unwrap()
    }
}

pub struct CoreInfo {
    pub system_name: &'static str,
    pub abbrev: &'static str,
    pub file_extensions: &'static [&'static str],
}

pub trait EmulatorCore {
    type Config: ConfigUi + Serialize + DeserializeOwned + Default;

    fn core_info() -> &'static CoreInfo;

    fn try_from_file(data: &[u8], backup: Option<&[u8]>, config: &Self::Config) -> Result<Self>
    where
        Self: Sized;
    fn game_info(&self) -> Vec<(String, String)>;

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

pub trait EmulatorCoreWrap: Sync + Send {
    fn core_info(&self) -> &CoreInfo;
    fn game_info(&self) -> Vec<(String, String)>;

    fn exec_frame(&mut self);
    fn reset(&mut self);

    fn frame_buffer(&self) -> &FrameBuffer;
    fn audio_buffer(&self) -> &AudioBuffer;
    fn set_input(&mut self, input: &InputData);

    fn backup(&self) -> Option<Vec<u8>>;

    fn save_state(&self) -> Vec<u8>;
    fn load_state(&mut self, data: &[u8]) -> Result<()>;
}

impl<T: EmulatorCore + Sync + Send> EmulatorCoreWrap for T {
    fn core_info(&self) -> &CoreInfo {
        T::core_info()
    }
    fn game_info(&self) -> Vec<(String, String)> {
        self.game_info()
    }

    fn exec_frame(&mut self) {
        self.exec_frame();
    }
    fn reset(&mut self) {
        self.reset();
    }
    fn frame_buffer(&self) -> &FrameBuffer {
        self.frame_buffer()
    }
    fn audio_buffer(&self) -> &AudioBuffer {
        self.audio_buffer()
    }
    fn set_input(&mut self, input: &InputData) {
        self.set_input(input);
    }
    fn backup(&self) -> Option<Vec<u8>> {
        self.backup()
    }
    fn save_state(&self) -> Vec<u8> {
        self.save_state()
    }
    fn load_state(&mut self, data: &[u8]) -> Result<()> {
        self.load_state(data)
    }
}

pub struct Emulator {
    pub core: Box<dyn EmulatorCoreWrap>,
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
        ARCHIVE_EXTENSIONS
            .iter()
            .find(|r| **r == ext.as_ref())
            .is_some()
    })
}

fn make_core_from_data<
    T: EmulatorCore + EmulatorCoreWrap + Sized + 'static,
    F: FnMut() -> Result<Vec<u8>>,
>(
    name: &str,
    ext: &str,
    mut data: F,
    config: &Config,
) -> Result<Box<dyn EmulatorCoreWrap>> {
    let core_info = <T as EmulatorCore>::core_info();
    if core_info.file_extensions.contains(&ext) {
        let backup = load_backup(core_info.abbrev, name, &config.save_dir)?;
        let data = data()?;
        let core = T::try_from_file(&data, backup.as_deref(), &config.core_config::<T>())?;
        Ok(Box::new(core))
    } else {
        bail!("Unsupported file extension: {ext}");
    }
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

    macro_rules! try_make {
        ($core:path) => {
            if let Ok(core) = make_core_from_data::<$core, _>(&name, &ext, &mut data, config) {
                return Ok(Emulator {
                    core,
                    game_name: name.to_string(),
                    auto_saved_states: VecDeque::new(),
                    total_auto_saved_size: 0,
                    prev_auto_saved_frame: 0,
                    prev_backup_saved_frame: 0,
                    save_dir: config.save_dir.clone(),
                    frames: 0,
                });
            }
        };
    }

    try_make!(gb::GameBoyCore);
    try_make!(gba::GameBoyAdvanceCore);

    bail!("No supported core for: {}", path.display())
}

impl Emulator {
    pub fn core_infos() -> Vec<&'static CoreInfo> {
        let mut ret = vec![];

        macro_rules! add {
            ($core:path) => {
                ret.push(<$core as EmulatorCore>::core_info());
            };
        }

        add!(gb::GameBoyCore);
        add!(gba::GameBoyAdvanceCore);

        ret
    }

    pub fn config_ui(ui: &mut egui::Ui, abbrev: &str, config: &mut Config) {
        macro_rules! add {
            ($core:path) => {
                if <$core as EmulatorCore>::core_info().abbrev == abbrev {
                    let mut core_config = config.core_config::<$core>();
                    core_config.ui(ui);
                    config.set_core_config::<$core>(core_config);
                }
            };
        }

        add!(gb::GameBoyCore);
        add!(gba::GameBoyAdvanceCore);
    }

    pub fn default_key_config(abbrev: &str) -> KeyConfig {
        macro_rules! add {
            ($core:path) => {
                if <$core as EmulatorCore>::core_info().abbrev == abbrev {
                    return <$core as EmulatorCore>::default_key_config();
                }
            };
        }

        add!(gb::GameBoyCore);
        add!(gba::GameBoyAdvanceCore);

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

fn frame_buffer_to_image(frame_buffer: &FrameBuffer) -> Image {
    let width = frame_buffer.width;
    let height = frame_buffer.height;

    let mut data = vec![0; width * height * 4];

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

    Image::new(
        Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
    )
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

struct GameScreen(pub Handle<Image>);

fn setup_emulator_system(
    mut commands: Commands,
    emulator: Res<Emulator>,
    mut images: ResMut<Assets<Image>>,
    mut event: EventWriter<WindowControlEvent>,
) {
    let width = emulator.core.frame_buffer().width as u32;
    let height = emulator.core.frame_buffer().height as u32;
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

    event.send(WindowControlEvent::Restore);
}

fn resume_emulator_system(mut event: EventWriter<WindowControlEvent>) {
    event.send(WindowControlEvent::Restore);
}

fn exit_emulator_system(mut commands: Commands, screen_entity: Query<Entity, With<ScreenSprite>>) {
    commands.entity(screen_entity.single()).despawn();
}

fn emulator_system(
    mut commands: Commands,
    screen: Res<GameScreen>,
    camera: Query<(Entity, &TiledCamera), With<TiledCamera>>,
    config: Res<Config>,
    mut emulator: ResMut<Emulator>,
    mut images: ResMut<Assets<Image>>,
    input: Res<InputData>,
    audio_queue: Res<AudioStreamQueue>,
    is_turbo: Res<hotkey::IsTurbo>,
) {
    emulator.core.set_input(&*input);

    {
        let camera = camera.single();
        let fb = emulator.core.frame_buffer();
        if (camera.1.width, camera.1.height) != (fb.width, fb.height) {
            commands.entity(camera.0).despawn();

            commands
                .spawn_bundle(
                    TiledCameraBundle::new()
                        .with_target_resolution(1, [fb.width as u32, fb.height as u32]),
                )
                .insert(TiledCamera {
                    width: fb.width,
                    height: fb.height,
                });
        }
    }

    let samples_per_frame = 48000 / 60;

    let mut queue = audio_queue.queue.lock().unwrap();

    let push_audio_queue = |queue: &mut VecDeque<AudioSample>, audio_buffer: &AudioBuffer| {
        for sample in &audio_buffer.samples {
            queue.push_back(sample.clone());
        }
    };

    if !is_turbo.0 {
        if queue.len() > samples_per_frame * 4 {
            // execution too fast. wait 1 frame.
            return;
        }

        let mut exec_frame = |queue: &mut VecDeque<AudioSample>| {
            emulator.core.exec_frame();
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
            push_audio_queue(&mut *queue, emulator.core.audio_buffer());
        };

        if queue.len() < samples_per_frame * 2 {
            // execution too slow. run 2 frame for supply enough audio samples.
            exec_frame(&mut *queue);
        }
        exec_frame(&mut *queue);

        // Update texture
        let fb = emulator.core.frame_buffer();
        let image = images.get_mut(&screen.0).unwrap();
        copy_frame_buffer(&mut image.data, fb);
    } else {
        for _ in 0..config.frame_skip_on_turbo {
            emulator.core.exec_frame();
            if queue.len() < samples_per_frame * 2 {
                push_audio_queue(&mut *queue, emulator.core.audio_buffer());
            }
        }
        // Update texture
        let fb = emulator.core.frame_buffer();
        let image = images.get_mut(&screen.0).unwrap();
        copy_frame_buffer(&mut image.data, fb);
        emulator.frames += 1;
    }

    if emulator.prev_backup_saved_frame + 60 * 60 <= emulator.frames {
        emulator.save_backup().unwrap();
    }
}

fn copy_frame_buffer(data: &mut [u8], frame_buffer: &FrameBuffer) {
    let width = frame_buffer.width;
    let height = frame_buffer.height;

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
