use anyhow::Result;
use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    input::{mouse::MouseButtonInput, ElementState},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::{PresentMode, WindowMode},
};
use bevy_easings::EasingsPlugin;
use bevy_egui::EguiPlugin;
use bevy_tiled_camera::TiledCameraPlugin;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{error, info, log_enabled};
use std::{
    cmp::min,
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use tgbr_core::{AudioBuffer, Config, FrameBuffer, GameBoy, Input as GameBoyInput};

use crate::{
    config::{self, load_config, load_persistent_state},
    file::{
        load_backup_ram, load_rom, load_state_data, print_rom_info, save_backup_ram,
        save_state_data,
    },
    hotkey,
    input::gameboy_input_system,
    menu,
    rewinding::{self, AutoSavedState},
};

pub fn main(rom_file: Option<PathBuf>) -> Result<()> {
    let config = load_config()?;

    let mut app = App::new();
    app.insert_resource(WindowDescriptor {
        title: "MERU".to_string(),
        resizable: false,
        present_mode: PresentMode::Fifo,
        width: menu::MENU_WIDTH as f32,
        height: menu::MENU_HEIGHT as f32,
        ..Default::default()
    })
    .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
    .init_resource::<UiState>()
    .init_resource::<FullscreenState>()
    .insert_resource(Msaa { samples: 4 })
    .insert_resource(bevy::log::LogSettings {
        level: bevy::utils::tracing::Level::INFO,
        filter: "tgba_core".to_string(),
    })
    .add_plugins(DefaultPlugins)
    .add_plugin(FrameTimeDiagnosticsPlugin)
    .add_plugin(TiledCameraPlugin)
    .add_plugin(EasingsPlugin)
    .add_plugin(EguiPlugin)
    .add_plugin(hotkey::HotKeyPlugin)
    .add_plugin(menu::MenuPlugin)
    .add_plugin(GameBoyPlugin)
    .add_plugin(rewinding::RewindingPlugin)
    .add_plugin(FpsPlugin)
    .add_plugin(MessagePlugin)
    .add_event::<WindowControlEvent>()
    .add_system(window_control_event)
    .insert_resource(LastClicked(0.0))
    .add_system(process_double_click)
    .add_startup_system(setup_audio.exclusive_system())
    .add_startup_system(setup)
    .add_startup_stage("single-startup", SystemStage::single_threaded())
    .add_startup_system_to_stage("single-startup", set_window_icon);

    if let Some(rom_file) = rom_file {
        let gb = GameBoyState::new(rom_file, &config)?;
        app.insert_resource(gb);
        app.add_state(AppState::Running);
    } else {
        app.add_state(AppState::Menu);
    }

    app.insert_resource(config);
    app.insert_resource(load_persistent_state()?);

    app.run();
    Ok(())
}

#[derive(Component)]
pub struct PixelFont;

fn setup(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    use bevy_tiled_camera::*;
    commands.spawn_bundle(TiledCameraBundle::new().with_target_resolution(1, [160, 144]));

    let pixel_font =
        Font::try_from_bytes(include_bytes!("../assets/fonts/PixelMplus12-Regular.ttf").to_vec())
            .unwrap();
    commands
        .spawn()
        .insert(fonts.add(pixel_font))
        .insert(PixelFont);
}

#[cfg(target_os = "windows")]
fn set_window_icon(windows: NonSend<bevy::winit::WinitWindows>) {
    use winit::window::Icon;

    const ICON_DATA: &[u8] = include_bytes!("../assets/tgbr.ico");
    const ICON_WIDTH: u32 = 64;
    const ICON_HEIGHT: u32 = 64;

    let primary = windows
        .get_window(bevy::window::WindowId::primary())
        .unwrap();

    let icon_rgba = image::load_from_memory_with_format(ICON_DATA, image::ImageFormat::Ico)
        .unwrap()
        .resize(
            ICON_WIDTH,
            ICON_HEIGHT,
            image::imageops::FilterType::Lanczos3,
        )
        .into_rgba8()
        .into_raw();

    let icon = Icon::from_rgba(icon_rgba, ICON_WIDTH, ICON_HEIGHT).unwrap();
    primary.set_window_icon(Some(icon));
}

#[cfg(not(target_os = "windows"))]
fn set_window_icon() {}

fn setup_audio(world: &mut World) {
    let audio_queue = Arc::new(Mutex::new(VecDeque::new()));
    world.insert_resource(AudioStreamQueue {
        queue: Arc::clone(&audio_queue),
    });

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("No audio output device available");

    let output_stream = device
        .build_output_stream(
            &cpal::StreamConfig {
                channels: 2,
                sample_rate: cpal::SampleRate(48000),
                buffer_size: cpal::BufferSize::Default,
            },
            move |data: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                assert!(data.len() % 2 == 0);

                let mut lock = audio_queue.lock().unwrap();

                for i in (0..data.len()).step_by(2) {
                    if let Some((l, r)) = lock.pop_front() {
                        data[i] = l;
                        data[i + 1] = r;
                    } else {
                        data[i] = 0;
                        data[i + 1] = 0;
                    }
                }
            },
            |err| panic!("Audio error: {err:#?}"),
        )
        .unwrap();

    output_stream.play().unwrap();

    world.insert_non_send_resource(output_stream);
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AppState {
    Menu,
    Running,
    Rewinding,
}

pub struct GameBoyState {
    pub gb: GameBoy,
    pub rom_file: PathBuf,
    pub save_dir: PathBuf,
    frames: usize,
    pub auto_saved_states: VecDeque<AutoSavedState>,
}

impl GameBoyState {
    pub fn new(rom_file: impl AsRef<Path>, config: &crate::config::Config) -> Result<Self> {
        let rom = load_rom(rom_file.as_ref())?;
        if log_enabled!(log::Level::Info) {
            print_rom_info(&rom.info());
        }

        let save_dir = config.save_dir().to_owned();
        let backup_ram = load_backup_ram(rom_file.as_ref(), &save_dir)?;

        let config = Config::default()
            .set_model(config.model())
            .set_dmg_palette(config.palette().get_palette())
            .set_boot_rom(config.boot_roms());

        let gb = GameBoy::new(rom, backup_ram, &config)?;

        Ok(Self {
            gb,
            rom_file: rom_file.as_ref().to_owned(),
            save_dir,
            frames: 0,
            auto_saved_states: VecDeque::new(),
        })
    }

    pub fn save_state(&self, slot: usize, config: &config::Config) -> Result<()> {
        let data = self.gb.save_state();
        save_state_data(&self.rom_file, slot, &data, config.state_dir())
    }

    pub fn load_state(&mut self, slot: usize, config: &config::Config) -> Result<()> {
        let data = load_state_data(&self.rom_file, slot, config.state_dir())?;
        self.gb.load_state(&data)
    }
}

impl Drop for GameBoyState {
    fn drop(&mut self) {
        if let Some(ram) = self.gb.backup_ram() {
            if let Err(err) = save_backup_ram(&self.rom_file, &ram, &self.save_dir) {
                error!("Failed to save backup ram: {err}");
            }
        } else {
            info!("No backup RAM to save");
        }
    }
}

struct GameBoyPlugin;

impl Plugin for GameBoyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameBoyInput>()
            .add_system_set(
                SystemSet::on_update(AppState::Running)
                    .with_system(gameboy_input_system.label("input")),
            )
            .add_system_set(
                SystemSet::on_enter(AppState::Running).with_system(setup_gameboy_system),
            )
            .add_system_set(
                SystemSet::on_resume(AppState::Running).with_system(resume_gameboy_system),
            )
            .add_system_set(
                SystemSet::on_update(AppState::Running)
                    .with_system(gameboy_system)
                    .after("input"),
            )
            .add_system_set(SystemSet::on_exit(AppState::Running).with_system(exit_gameboy_system));
    }
}

pub struct GameScreen(Handle<Image>);

#[derive(Debug, Default)]
struct AudioStreamQueue {
    queue: Arc<Mutex<VecDeque<(i16, i16)>>>,
}

// impl AudioStream for AudioStreamQueue {
//     fn next(&mut self, _: f64) -> Frame {
//         let mut buffer = self.queue.lock().unwrap();
//         buffer.pop_front().unwrap_or(Frame {
//             left: 0.0,
//             right: 0.0,
//         })
//     }
// }

#[derive(Default)]
pub struct UiState {
    pub state_save_slot: usize,
}

#[derive(Component)]
pub struct ScreenSprite;

fn setup_gameboy_system(
    mut commands: Commands,
    gb_state: Res<GameBoyState>,
    mut images: ResMut<Assets<Image>>,
    mut event: EventWriter<WindowControlEvent>,
) {
    let width = gb_state.gb.frame_buffer().width as u32;
    let height = gb_state.gb.frame_buffer().height as u32;
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

fn resume_gameboy_system(mut event: EventWriter<WindowControlEvent>) {
    event.send(WindowControlEvent::Restore);
}

fn exit_gameboy_system(mut commands: Commands, screen_entity: Query<Entity, With<ScreenSprite>>) {
    commands.entity(screen_entity.single()).despawn();
}

#[derive(Default)]
pub struct FullscreenState(pub bool);

pub enum WindowControlEvent {
    ToggleFullscreen,
    ChangeScale(usize),
    Restore,
}

fn window_control_event(
    mut windows: ResMut<Windows>,
    mut event: EventReader<WindowControlEvent>,
    mut fullscreen_state: ResMut<FullscreenState>,
    mut config: ResMut<config::Config>,
    app_state: Res<State<AppState>>,
) {
    let running = app_state.current() == &AppState::Running;

    for event in event.iter() {
        match event {
            WindowControlEvent::ToggleFullscreen => {
                let window = windows.get_primary_mut().unwrap();
                fullscreen_state.0 = !fullscreen_state.0;

                if fullscreen_state.0 {
                    window.set_mode(WindowMode::BorderlessFullscreen);
                } else {
                    window.set_mode(WindowMode::Windowed);
                }
                if running {
                    let window = windows.get_primary_mut().unwrap();
                    restore_window(window, fullscreen_state.0, config.scaling());
                }
            }
            WindowControlEvent::ChangeScale(scale) => {
                config.set_scaling(*scale);
                if running {
                    let window = windows.get_primary_mut().unwrap();
                    restore_window(window, fullscreen_state.0, config.scaling());
                }
            }
            WindowControlEvent::Restore => {
                let window = windows.get_primary_mut().unwrap();
                restore_window(window, fullscreen_state.0, config.scaling());
            }
        }
    }
}

struct LastClicked(f64);

fn process_double_click(
    time: Res<Time>,
    mut last_clicked: ResMut<LastClicked>,
    mut mouse_button_event: EventReader<MouseButtonInput>,
    mut window_control_event: EventWriter<WindowControlEvent>,
) {
    for ev in mouse_button_event.iter() {
        if ev.button == MouseButton::Left && ev.state == ElementState::Pressed {
            let cur = time.seconds_since_startup();
            let diff = cur - last_clicked.0;

            if diff < 0.25 {
                window_control_event.send(WindowControlEvent::ToggleFullscreen);
                last_clicked.0 = cur - 1.0;
            } else {
                last_clicked.0 = cur;
            }
        }
    }
}

fn restore_window(window: &mut Window, fullscreen: bool, scaling: usize) {
    let width = 160;
    let height = 144;

    if !fullscreen {
        let scale = scaling as f32;
        window.set_resolution(width as f32 * scale, height as f32 * scale);
    }
}

fn gameboy_system(
    screen: Res<GameScreen>,
    config: Res<config::Config>,
    mut state: ResMut<GameBoyState>,
    mut images: ResMut<Assets<Image>>,
    input: Res<GameBoyInput>,
    audio_queue: Res<AudioStreamQueue>,
    is_turbo: Res<hotkey::IsTurbo>,
) {
    state.gb.set_input(&*input);

    let samples_per_frame = 48000 / 60;

    let mut queue = audio_queue.queue.lock().unwrap();

    let push_audio_queue = |queue: &mut VecDeque<(i16, i16)>, audio_buffer: &AudioBuffer| {
        for sample in &audio_buffer.buf {
            queue.push_back((sample.left, sample.right));
        }
    };

    let cc = make_color_correction(state.gb.model().is_cgb() && config.color_correction());

    if !is_turbo.0 {
        if queue.len() > samples_per_frame * 4 {
            // execution too fast. wait 1 frame.
            return;
        }

        let mut exec_frame = |queue: &mut VecDeque<(i16, i16)>| {
            state.gb.exec_frame();
            if state.frames % config.auto_state_save_freq() == 0 {
                let saved_state = AutoSavedState {
                    data: state.gb.save_state(),
                    thumbnail: cc.frame_buffer_to_image(state.gb.frame_buffer()),
                };

                state.auto_saved_states.push_back(saved_state);
                if state.auto_saved_states.len() > config.auto_state_save_limit() {
                    state.auto_saved_states.pop_front();
                }
            }
            push_audio_queue(&mut *queue, state.gb.audio_buffer());
            state.frames += 1;
        };

        if queue.len() < samples_per_frame * 2 {
            // execution too slow. run 2 frame for supply enough audio samples.
            exec_frame(&mut *queue);
        }
        exec_frame(&mut *queue);

        // Update texture
        let fb = state.gb.frame_buffer();
        let image = images.get_mut(&screen.0).unwrap();
        cc.copy_frame_buffer(&mut image.data, fb);
    } else {
        for _ in 0..config.frame_skip_on_turbo() {
            state.gb.exec_frame();
            if queue.len() < samples_per_frame * 2 {
                push_audio_queue(&mut *queue, state.gb.audio_buffer());
            }
        }
        // Update texture
        let fb = state.gb.frame_buffer();
        let image = images.get_mut(&screen.0).unwrap();
        cc.copy_frame_buffer(&mut image.data, fb);
        state.frames += 1;
    }
}

pub fn make_color_correction(color_correction: bool) -> Box<dyn ColorCorrection> {
    if color_correction {
        Box::new(CorrectColor) as Box<dyn ColorCorrection>
    } else {
        Box::new(RawColor) as Box<dyn ColorCorrection>
    }
}

pub trait ColorCorrection {
    fn translate(&self, c: &tgbr_core::Color) -> tgbr_core::Color;

    fn frame_buffer_to_image(&self, frame_buffer: &FrameBuffer) -> Image {
        let width = frame_buffer.width as u32;
        let height = frame_buffer.height as u32;

        let mut data = vec![0; width as usize * height as usize * 4];
        self.copy_frame_buffer(&mut data, frame_buffer);
        Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
        )
    }

    fn copy_frame_buffer(&self, data: &mut [u8], frame_buffer: &FrameBuffer) {
        let width = frame_buffer.width;
        let height = frame_buffer.height;

        for y in 0..height {
            for x in 0..width {
                let ix = y * width + x;
                let pixel = &mut data[ix * 4..ix * 4 + 4];
                let c = self.translate(&frame_buffer.buf[ix]);
                pixel[0] = c.r;
                pixel[1] = c.g;
                pixel[2] = c.b;
                pixel[3] = 0xff;
            }
        }
    }
}

struct RawColor;

impl ColorCorrection for RawColor {
    fn translate(&self, c: &tgbr_core::Color) -> tgbr_core::Color {
        *c
    }
}

struct CorrectColor;

impl ColorCorrection for CorrectColor {
    fn translate(&self, c: &tgbr_core::Color) -> tgbr_core::Color {
        let r = c.r as u16;
        let g = c.g as u16;
        let b = c.b as u16;
        tgbr_core::Color {
            r: min(240, ((r * 26 + g * 4 + b * 2) / 32) as u8),
            g: min(240, ((g * 24 + b * 8) / 32) as u8),
            b: min(240, ((r * 6 + g * 4 + b * 22) / 32) as u8),
        }
    }
}

struct FpsPlugin;

impl Plugin for FpsPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(SystemSet::on_enter(AppState::Running).with_system(setup_fps_system))
            .add_system_set(SystemSet::on_exit(AppState::Running).with_system(exit_fps_system))
            .add_system_set(SystemSet::on_update(AppState::Running).with_system(fps_system));
    }
}

#[derive(Component)]
pub struct FpsText;

#[derive(Component)]
pub struct FpsTextBg;

fn setup_fps_system(mut commands: Commands, pixel_font: Query<&Handle<Font>, With<PixelFont>>) {
    let pixel_font = pixel_font.single();

    commands
        .spawn_bundle(Text2dBundle {
            text: Text::with_section(
                "",
                TextStyle {
                    font: pixel_font.clone(),
                    font_size: 24.0,
                    color: Color::WHITE,
                },
                TextAlignment::default(),
            ),
            transform: Transform::from_xyz(52.0, 72.0, 2.0).with_scale(Vec3::splat(0.5)),
            ..Default::default()
        })
        .insert(FpsText);

    commands
        .spawn_bundle(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.0, 0.0, 0.0, 0.75),
                custom_size: Some(Vec2::new(30.0, 12.0)),
                ..Default::default()
            },
            transform: Transform::from_xyz(65.0, 66.0, 1.0),
            ..Default::default()
        })
        .insert(FpsTextBg);
}

fn exit_fps_system(
    mut commands: Commands,

    fps_text: Query<Entity, With<FpsText>>,
    fps_text_bg: Query<Entity, With<FpsTextBg>>,
) {
    commands.entity(fps_text.single()).despawn();
    commands.entity(fps_text_bg.single()).despawn();
}

#[allow(clippy::type_complexity)]
fn fps_system(
    config: Res<config::Config>,
    diagnostics: ResMut<Diagnostics>,
    is_turbo: Res<hotkey::IsTurbo>,
    mut ps: ParamSet<(
        Query<(&mut Text, &mut Visibility), With<FpsText>>,
        Query<&mut Visibility, With<FpsTextBg>>,
    )>,
) {
    let mut p0 = ps.p0();
    let (mut text, mut visibility) = p0.single_mut();
    visibility.is_visible = config.show_fps();
    let fps_diag = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS).unwrap();
    let fps = fps_diag.value().unwrap_or(0.0)
        * if is_turbo.0 {
            config.frame_skip_on_turbo() as f64
        } else {
            1.0
        };
    let fps = format!("{fps:5.02}");
    text.sections[0].value = fps.chars().take(5).collect();

    let mut p1 = ps.p1();
    let mut visibility_bg = p1.single_mut();
    visibility_bg.is_visible = config.show_fps();
}

struct MessagePlugin;

impl Plugin for MessagePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(message_event_system)
            .add_system(message_update_system)
            .add_event::<ShowMessage>();
    }
}

pub struct ShowMessage(pub String);

#[derive(Component)]
struct MessageText {
    start: f64,
}

fn message_event_system(
    mut commands: Commands,
    time: Res<Time>,
    mut event: EventReader<ShowMessage>,
    pixel_font: Query<&Handle<Font>, With<PixelFont>>,
    mut messages: Query<(Entity, &Transform), With<MessageText>>,
) {
    let pixel_font = pixel_font.single();

    for ShowMessage(msg) in event.iter() {
        for (entity, trans) in messages.iter_mut() {
            use bevy_easings::*;

            commands.entity(entity).insert(trans.ease_to(
                Transform::from_xyz(0.0, 15.0, 0.0) * *trans,
                EaseFunction::CubicInOut,
                EasingType::Once {
                    duration: std::time::Duration::from_millis(100),
                },
            ));
        }

        commands
            .spawn_bundle(Text2dBundle {
                text: Text::with_section(
                    msg,
                    TextStyle {
                        font: pixel_font.clone(),
                        font_size: 12.0,
                        color: Color::WHITE,
                    },
                    TextAlignment::default(),
                ),
                transform: Transform::from_xyz(-80.0, -60.0, 2.0),
                ..Default::default()
            })
            .insert(MessageText {
                start: time.seconds_since_startup(),
            })
            .with_children(|parent| {
                parent.spawn_bundle(SpriteBundle {
                    sprite: Sprite {
                        color: Color::rgba(0.0, 0.0, 0.0, 0.75),
                        custom_size: Some(Vec2::new(160.0, 12.0)),
                        ..Default::default()
                    },
                    transform: Transform::from_xyz(80.0, -6.0, -1.0),
                    ..Default::default()
                });
            });
    }
}

fn message_update_system(
    mut commands: Commands,
    time: Res<Time>,
    messages: Query<(Entity, &MessageText), With<MessageText>>,
) {
    for (entity, msg) in messages.iter() {
        if time.seconds_since_startup() - msg.start > 3.0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}
