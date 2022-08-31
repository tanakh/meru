use bevy::prelude::*;
use bevy_easings::*;
use std::time::Duration;

use crate::{
    app::{AppState, ScreenSprite},
    config::{self, SystemKey},
    core::Emulator,
    hotkey::HotKey,
    input::InputState,
};

#[derive(Clone)]
pub struct AutoSavedState {
    pub thumbnail: Image,
    pub data: Vec<u8>,
}

impl AutoSavedState {
    pub fn size(&self) -> usize {
        self.data.len() + self.thumbnail.data.len()
    }
}

pub struct RewindingState {
    pos: usize,
    load_pos: Option<usize>,
    exit: bool,
}

pub struct RewindingPlugin;

impl Plugin for RewindingPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            SystemSet::on_enter(AppState::Rewinding).with_system(enter_rewinding_system),
        )
        .add_system_set(SystemSet::on_update(AppState::Rewinding).with_system(rewinding_system))
        .add_system_set(SystemSet::on_exit(AppState::Rewinding).with_system(exit_rewinding_system));
    }
}

#[derive(Component)]
struct BgColor;

#[derive(Component)]
struct Preview;

#[derive(Component)]
struct Thumbnail(usize);

fn enter_rewinding_system(
    mut commands: Commands,
    emulator: ResMut<Emulator>,
    mut images: ResMut<Assets<Image>>,
    mut screen_visibility: Query<&mut Visibility, With<ScreenSprite>>,
) {
    let screen_width = emulator.core.frame_buffer().width as f32;
    let screen_height = emulator.core.frame_buffer().height as f32;

    for mut visibility in screen_visibility.iter_mut() {
        visibility.is_visible = false;
    }

    let state_num = emulator.auto_saved_states.len();
    assert!(state_num > 0);

    let preview_image = images.add(emulator.auto_saved_states[state_num - 1].thumbnail.clone());

    commands
        .spawn_bundle(SpriteBundle {
            sprite: Sprite {
                color: Color::GRAY,
                custom_size: Some(Vec2::new(screen_width, screen_height)),
                ..Default::default()
            },
            transform: Transform::from_xyz(0.0, 0.0, -0.01),
            ..Default::default()
        })
        .insert(BgColor);

    commands
        .spawn_bundle(SpriteBundle {
            texture: preview_image,
            transform: Transform::from_xyz(0.0, 0.0, 1.0),
            ..Default::default()
        })
        .insert(
            Transform {
                ..Default::default()
            }
            .ease_to(
                Transform::from_xyz(0.0, screen_height / 6.0, 1.0)
                    .with_scale(Vec3::splat(2.0 / 3.0)),
                EaseFunction::CubicInOut,
                EasingType::Once {
                    duration: Duration::from_millis(200),
                },
            ),
        )
        .insert(Preview);

    for i in 0..4 {
        if state_num > i {
            let thumbnail = images.add(
                emulator.auto_saved_states[state_num - 1 - i]
                    .thumbnail
                    .clone(),
            );
            commands
                .spawn_bundle(SpriteBundle {
                    texture: thumbnail,
                    transform: Transform::from_xyz(
                        -(i as f32) * screen_width / 4.0,
                        -screen_height / 2.0 + screen_height / 6.0,
                        0.0,
                    )
                    .with_scale(Vec3::splat(1.0 / 4.5)),
                    ..Default::default()
                })
                .insert(Thumbnail(i));
        }
    }

    commands.insert_resource(RewindingState {
        pos: state_num - 1,
        load_pos: None,
        exit: false,
    });
}

#[allow(clippy::too_many_arguments)]
fn rewinding_system(
    mut commands: Commands,
    mut emulator: ResMut<Emulator>,
    mut app_state: ResMut<State<AppState>>,
    mut rewinding_state: ResMut<RewindingState>,
    mut preview: Query<(&mut Handle<Image>, &Transform, Entity), With<Preview>>,
    thumbnails: Query<(Entity, &Transform), With<Thumbnail>>,
    config: Res<config::Config>,
    input_keycode: Res<Input<KeyCode>>,
    mut images: ResMut<Assets<Image>>,
    input_gamepad_button: Res<Input<GamepadButton>>,
    input_gamepad_axis: Res<Axis<GamepadAxis>>,
    easing: Query<&EasingComponent<Transform>>,
) {
    let screen_width = emulator.core.frame_buffer().width as f32;
    let screen_height = emulator.core.frame_buffer().height as f32;

    let input_state = InputState::new(&input_keycode, &input_gamepad_button, &input_gamepad_axis);

    // wait for animation
    if easing.iter().next().is_some() {
        // remove invisible thumbnails
        for (entity, transform) in thumbnails.iter() {
            if transform.translation.x.abs() > screen_width {
                commands.entity(entity).despawn();
                // TODO: remove image from assets
            }
        }
        return;
    }

    if rewinding_state.exit {
        app_state.pop().unwrap();
        return;
    }

    if let Some(load_pos) = &rewinding_state.load_pos {
        while emulator.auto_saved_states.len() > *load_pos + 1 {
            emulator.auto_saved_states.pop_back();
        }
        let state = emulator.auto_saved_states.back().unwrap().clone();

        let mut preview = preview.single_mut();
        *preview.0 = images.add(state.thumbnail);
        commands.entity(preview.2).insert(preview.1.ease_to(
            Transform::from_xyz(0.0, 0.0, 1.0),
            EaseFunction::CubicInOut,
            EasingType::Once {
                duration: Duration::from_millis(200),
            },
        ));
        emulator.core.load_state(&state.data).unwrap();
        rewinding_state.exit = true;
        return;
    }

    let left = config.system_keys.pressed(&SystemKey::Left, &input_state);
    let right = config.system_keys.pressed(&SystemKey::Right, &input_state);

    if left || right {
        let mut do_move = false;
        if left && rewinding_state.pos > 0 {
            if rewinding_state.pos >= 4 {
                let ix = rewinding_state.pos - 4;
                let thumbnail = images.add(emulator.auto_saved_states[ix].thumbnail.clone());

                commands
                    .spawn_bundle(SpriteBundle {
                        texture: thumbnail,
                        transform: Transform::from_xyz(
                            -3.0 * screen_width / 4.0,
                            -screen_height / 2.0 + screen_height / 6.0,
                            0.0,
                        )
                        .with_scale(Vec3::splat(1.0 / 4.5)),
                        ..Default::default()
                    })
                    .insert(Thumbnail(ix));
            }

            rewinding_state.pos -= 1;
            do_move = true;
        }
        if right && rewinding_state.pos < emulator.auto_saved_states.len() - 1 {
            if rewinding_state.pos + 4 < emulator.auto_saved_states.len() {
                let ix = rewinding_state.pos + 4;
                let thumbnail = images.add(emulator.auto_saved_states[ix].thumbnail.clone());

                commands
                    .spawn_bundle(SpriteBundle {
                        texture: thumbnail,
                        transform: Transform::from_xyz(
                            3.0 * screen_width / 4.0,
                            -screen_height / 2.0 + screen_height / 6.0,
                            0.0,
                        )
                        .with_scale(Vec3::splat(1.0 / 4.5)),
                        ..Default::default()
                    })
                    .insert(Thumbnail(ix));
            }

            rewinding_state.pos += 1;
            do_move = true;
        }

        if do_move {
            let dx = if left { 1.0 } else { -1.0 } * screen_width / 4.0;
            for (entity, trans) in thumbnails.iter() {
                commands.entity(entity).insert(trans.ease_to(
                    Transform::from_xyz(dx, 0.0, 0.0) * *trans,
                    EaseFunction::CubicInOut,
                    EasingType::Once {
                        duration: Duration::from_millis(100),
                    },
                ));
            }

            *preview.single_mut().0 = images.add(
                emulator.auto_saved_states[rewinding_state.pos]
                    .thumbnail
                    .clone(),
            );
        }
    }

    if config
        .system_keys
        .just_pressed(&SystemKey::Ok, &input_state)
    {
        rewinding_state.load_pos = Some(rewinding_state.pos);
    } else if config
        .system_keys
        .just_pressed(&SystemKey::Cancel, &input_state)
        || config.hotkeys.just_pressed(&HotKey::Menu, &input_state)
    {
        rewinding_state.load_pos = Some(emulator.auto_saved_states.len() - 1);
    }
}

fn exit_rewinding_system(
    mut commands: Commands,
    bg_color: Query<Entity, With<BgColor>>,
    preview: Query<Entity, With<Preview>>,
    thumbnails: Query<Entity, With<Thumbnail>>,
    mut screen_visibility: Query<&mut Visibility, With<ScreenSprite>>,
) {
    for mut visibility in screen_visibility.iter_mut() {
        visibility.is_visible = true;
    }

    for entity in bg_color
        .iter()
        .chain(preview.iter())
        .chain(thumbnails.iter())
    {
        commands.entity(entity).despawn();
    }
}
