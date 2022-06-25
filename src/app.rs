use anyhow::Result;
use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    input::{mouse::MouseButtonInput, ElementState},
    prelude::*,
    window::{PresentMode, WindowMode},
};
use bevy_easings::EasingsPlugin;
use bevy_egui::{EguiContext, EguiPlugin};
use bevy_tiled_camera::TiledCameraPlugin;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use crate::{
    config::{self, load_config, load_persistent_state},
    core::{self, AudioSample, Emulator},
    hotkey, menu,
    rewinding::{self},
};

pub fn main() -> Result<()> {
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
        filter: "".to_string(),
    })
    .add_plugins(DefaultPlugins)
    .add_plugin(FrameTimeDiagnosticsPlugin)
    .add_plugin(TiledCameraPlugin)
    .add_plugin(EasingsPlugin)
    .add_plugin(EguiPlugin)
    .add_plugin(hotkey::HotKeyPlugin)
    .add_plugin(menu::MenuPlugin)
    .add_plugin(core::EmulatorPlugin)
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

    app.add_state(AppState::Menu);

    app.insert_resource(config);
    app.insert_resource(load_persistent_state()?);

    app.run();
    Ok(())
}

#[derive(Component)]
struct PixelFont;

#[derive(Component)]
pub struct TiledCamera {
    pub width: usize,
    pub height: usize,
}

fn setup(
    mut commands: Commands,
    mut fonts: ResMut<Assets<Font>>,
    mut egui_ctx: ResMut<EguiContext>,
) {
    use bevy_tiled_camera::*;
    commands
        .spawn_bundle(TiledCameraBundle::new().with_target_resolution(1, [320, 240]))
        .insert(TiledCamera {
            width: 320,
            height: 240,
        });

    let ctx = egui_ctx.ctx_mut();

    let mut style = (*ctx.style()).clone();

    for style in style.text_styles.iter_mut() {
        style.1.size *= 2.0;
    }

    ctx.set_style(style);

    let pixel_font =
        Font::try_from_bytes(include_bytes!("../assets/fonts/x8y12pxTheStrongGamer.ttf").to_vec())
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
                    if let Some(sample) = lock.pop_front() {
                        data[i] = sample.left;
                        data[i + 1] = sample.right;
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

#[derive(Default)]
pub struct AudioStreamQueue {
    pub queue: Arc<Mutex<VecDeque<AudioSample>>>,
}

#[derive(Default)]
pub struct UiState {
    pub state_save_slot: usize,
}

#[derive(Component)]
pub struct ScreenSprite;

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
    emulator: Option<Res<Emulator>>,
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
                    restore_window(
                        emulator.as_deref().unwrap(),
                        window,
                        fullscreen_state.0,
                        config.scaling(),
                    );
                }
            }
            WindowControlEvent::ChangeScale(scale) => {
                config.set_scaling(*scale);
                if running {
                    let window = windows.get_primary_mut().unwrap();
                    restore_window(
                        emulator.as_deref().unwrap(),
                        window,
                        fullscreen_state.0,
                        config.scaling(),
                    );
                }
            }
            WindowControlEvent::Restore => {
                let window = windows.get_primary_mut().unwrap();
                restore_window(
                    emulator.as_deref().unwrap(),
                    window,
                    fullscreen_state.0,
                    config.scaling(),
                );
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

fn restore_window(emulator: &Emulator, window: &mut Window, fullscreen: bool, scaling: usize) {
    let width = emulator.core.frame_buffer().width;
    let height = emulator.core.frame_buffer().height;

    if !fullscreen {
        let scale = scaling as f32;
        window.set_resolution(width as f32 * scale, height as f32 * scale);
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
                    font_size: 12.0,
                    color: Color::WHITE,
                },
                TextAlignment::default(),
            ),
            transform: Transform::from_xyz(0.0, 0.0, 2.0),
            ..Default::default()
        })
        .insert(FpsText);

    commands
        .spawn_bundle(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.0, 0.0, 0.0, 0.75),
                custom_size: Some(Vec2::new(42.0, 12.0)),
                ..Default::default()
            },
            transform: Transform::from_xyz(0.0, 0.0, 1.0),
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
    emulator: Option<Res<Emulator>>,
    mut ps: ParamSet<(
        Query<(&mut Text, &mut Visibility, &mut Transform), With<FpsText>>,
        Query<(&mut Visibility, &mut Transform), With<FpsTextBg>>,
    )>,
) {
    let emulator = if let Some(emulator) = emulator {
        emulator
    } else {
        return;
    };

    let screen_width = emulator.core.frame_buffer().width;
    let screen_height = emulator.core.frame_buffer().height;

    let mut p0 = ps.p0();
    let (mut text, mut visibility, mut transform) = p0.single_mut();
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
    *transform = Transform::from_xyz(
        (screen_width / 2 - 40) as _,
        (screen_height / 2 + 2) as _,
        2.0,
    );

    let mut p1 = ps.p1();
    let (mut visibility, mut transform) = p1.single_mut();
    visibility.is_visible = config.show_fps();
    *transform = Transform::from_xyz(
        (screen_width / 2 - 21) as _,
        (screen_height / 2 - 6) as _,
        1.0,
    );
}

struct MessagePlugin;

impl Plugin for MessagePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(message_event_system.label("message_event"))
            .add_system(message_update_system.after("message_event"))
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
    emulator: Option<Res<Emulator>>,
    mut event: EventReader<ShowMessage>,
    pixel_font: Query<&Handle<Font>, With<PixelFont>>,
    mut messages: Query<(Entity, &Transform), With<MessageText>>,
) {
    let emulator = if let Some(emulator) = emulator {
        emulator
    } else {
        return;
    };
    let screen_width = emulator.core.frame_buffer().width as f32;
    let screen_height = emulator.core.frame_buffer().height as f32;

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
                transform: Transform::from_xyz(
                    -screen_width / 2.0 + 2.0,
                    -screen_height / 2.0 + 20.0,
                    2.0,
                ),
                ..Default::default()
            })
            .insert(MessageText {
                start: time.seconds_since_startup(),
            })
            .with_children(|parent| {
                parent.spawn_bundle(SpriteBundle {
                    sprite: Sprite {
                        color: Color::rgba(0.0, 0.0, 0.0, 0.75),
                        custom_size: Some(Vec2::new(screen_width, 12.0)),
                        ..Default::default()
                    },
                    transform: Transform::from_xyz(screen_width / 2.0 - 2.0, -8.0, -1.0),
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
