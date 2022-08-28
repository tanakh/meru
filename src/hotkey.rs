use bevy::prelude::*;
use either::Either;
use enum_iterator::{all, Sequence};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use Either::{Left, Right};

use crate::{
    app::{AppState, ShowMessage, UiState, WindowControlEvent},
    config::Config,
    core::Emulator,
    input::{InputState, KeyConfig},
    utils::{spawn_local, unbounded_channel, Receiver, Sender},
};

pub struct HotKeyPlugin;

impl Plugin for HotKeyPlugin {
    fn build(&self, app: &mut App) {
        let (s, r) = unbounded_channel::<Either<HotKey, HotKeyCont>>();
        app.add_system(check_hotkey)
            .add_system(process_hotkey)
            .insert_resource(IsTurbo(false))
            .insert_resource(s)
            .insert_resource(r);
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, Sequence)]
pub enum HotKey {
    Reset,
    Turbo,
    StateSave,
    StateLoad,
    NextSlot,
    PrevSlot,
    Rewind,
    Menu,
    FullScreen,
    ScaleUp,
    ScaleDown,
}

enum HotKeyCont {
    StateLoadDone(anyhow::Result<Vec<u8>>),
}

impl Display for HotKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            HotKey::Reset => "Reset",
            HotKey::Turbo => "Turbo",
            HotKey::StateSave => "State Save",
            HotKey::StateLoad => "State Load",
            HotKey::NextSlot => "State Slot Next",
            HotKey::PrevSlot => "State Slot Prev",
            HotKey::Rewind => "Start Rewindng",
            HotKey::Menu => "Enter/Leave Menu",
            HotKey::FullScreen => "Fullsceen",
            HotKey::ScaleUp => "Window Scale +",
            HotKey::ScaleDown => "Window Scale -",
        };
        write!(f, "{s}")
    }
}

pub type HotKeys = KeyConfig<HotKey>;

impl Default for HotKeys {
    fn default() -> Self {
        use meru_interface::key_assign::*;
        use HotKey::*;
        Self(vec![
            (Reset, all![keycode!(LControl), keycode!(R)]),
            (Turbo, any![keycode!(Tab), pad_button!(0, LeftTrigger2)]),
            (StateSave, all![keycode!(LControl), keycode!(S)]),
            (StateLoad, all![keycode!(LControl), keycode!(L)]),
            (NextSlot, all![keycode!(LControl), keycode!(N)]),
            (PrevSlot, all![keycode!(LControl), keycode!(P)]),
            (
                Rewind,
                any![
                    keycode!(Back),
                    all![pad_button!(0, LeftTrigger2), pad_button!(0, RightTrigger2)]
                ],
            ),
            (Menu, keycode!(Escape)),
            (FullScreen, all![keycode!(RAlt), keycode!(Return)]),
            (
                ScaleUp,
                all![keycode!(LControl), any![keycode!(Plus), keycode!(Equals)]],
            ),
            (ScaleDown, all![keycode!(LControl), keycode!(Minus)]),
        ])
    }
}

pub struct IsTurbo(pub bool);

fn check_hotkey(
    config: Res<Config>,
    input_keycode: Res<Input<KeyCode>>,
    input_gamepad_button: Res<Input<GamepadButton>>,
    input_gamepad_axis: Res<Axis<GamepadAxis>>,
    writer: Res<Sender<Either<HotKey, HotKeyCont>>>,
    mut is_turbo: ResMut<IsTurbo>,
) {
    let input_state = InputState::new(&input_keycode, &input_gamepad_button, &input_gamepad_axis);

    for hotkey in all::<HotKey>() {
        if config.hotkeys.just_pressed(&hotkey, &input_state) {
            writer.try_send(Left(hotkey)).unwrap();
        }
    }

    is_turbo.0 = config.hotkeys.pressed(
        &HotKey::Turbo,
        &InputState::new(&input_keycode, &input_gamepad_button, &input_gamepad_axis),
    );
}

#[allow(clippy::too_many_arguments)]
fn process_hotkey(
    mut config: ResMut<Config>,
    recv: Res<Receiver<Either<HotKey, HotKeyCont>>>,
    send: Res<Sender<Either<HotKey, HotKeyCont>>>,
    mut app_state: ResMut<State<AppState>>,
    mut emulator: Option<ResMut<Emulator>>,
    mut ui_state: ResMut<UiState>,
    mut window_control_event: EventWriter<WindowControlEvent>,
    mut message_event: EventWriter<ShowMessage>,
) {
    while let Ok(hotkey) = recv.try_recv() {
        match hotkey {
            Left(HotKey::Reset) => {
                if let Some(emulator) = &mut emulator {
                    emulator.reset();
                    message_event.send(ShowMessage("Reset machine".to_string()));
                }
            }
            Left(HotKey::StateSave) => {
                if let Some(emulator) = &emulator {
                    let fut = emulator.save_state_slot(ui_state.state_save_slot, config.as_ref());

                    spawn_local(async move { fut.await.unwrap() });

                    message_event.send(ShowMessage(format!(
                        "State saved: #{}",
                        ui_state.state_save_slot
                    )));
                }
            }
            Left(HotKey::StateLoad) => {
                if let Some(emulator) = &emulator {
                    let send = send.clone();

                    let fut = emulator.load_state_slot(ui_state.state_save_slot, config.as_ref());

                    spawn_local(async move {
                        let result = fut.await;
                        send.send(Right(HotKeyCont::StateLoadDone(result)))
                            .await
                            .unwrap();
                    });
                }
            }
            Right(HotKeyCont::StateLoadDone(data)) => {
                if let Some(emulator) = &mut emulator {
                    match data {
                        Ok(data) => {
                            if let Err(err) = emulator.load_state_data(&data) {
                                message_event
                                    .send(ShowMessage(format!("Failed to load state: {err:?}")));
                            } else {
                                message_event.send(ShowMessage(format!(
                                    "State loaded: #{}",
                                    ui_state.state_save_slot
                                )));
                            }
                        }
                        Err(err) => {
                            message_event
                                .send(ShowMessage(format!("Failed to load state: {err:?}")));
                        }
                    }
                }
            }
            Left(HotKey::NextSlot) => {
                ui_state.state_save_slot += 1;
                message_event.send(ShowMessage(format!(
                    "State slot changed: #{}",
                    ui_state.state_save_slot
                )));
            }
            Left(HotKey::PrevSlot) => {
                ui_state.state_save_slot = ui_state.state_save_slot.saturating_sub(1);
                message_event.send(ShowMessage(format!(
                    "State slot changed: #{}",
                    ui_state.state_save_slot
                )));
            }
            Left(HotKey::Rewind) => {
                if app_state.current() == &AppState::Running {
                    let emulator = emulator.as_mut().unwrap();
                    emulator.push_auto_save();
                    app_state.push(AppState::Rewinding).unwrap();
                }
            }
            Left(HotKey::Menu) => {
                if app_state.current() == &AppState::Running {
                    app_state.set(AppState::Menu).unwrap();
                } else if app_state.current() == &AppState::Menu && emulator.is_some() {
                    app_state.set(AppState::Running).unwrap();
                }
            }
            Left(HotKey::FullScreen) => {
                window_control_event.send(WindowControlEvent::ToggleFullscreen);
            }
            Left(HotKey::ScaleUp) => {
                config.scaling += 1;
                window_control_event.send(WindowControlEvent::Restore);
            }
            Left(HotKey::ScaleDown) => {
                config.scaling = (config.scaling - 1).max(1);
                window_control_event.send(WindowControlEvent::Restore);
            }

            Left(HotKey::Turbo) => {}
        }
    }
}
