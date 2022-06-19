use bevy::prelude::*;
use enum_iterator::{all, Sequence};
use serde::{Deserialize, Serialize};
use std::{cmp::max, fmt::Display};

use crate::{
    app::{
        make_color_correction, AppState, GameBoyState, ShowMessage, UiState, WindowControlEvent,
    },
    config::Config,
    key_assign::*,
    rewinding::AutoSavedState,
};

pub struct HotKeyPlugin;

impl Plugin for HotKeyPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(check_hotkey)
            .add_system(process_hotkey)
            .add_event::<HotKey>()
            .insert_resource(IsTurbo(false));
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

impl Display for HotKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
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
            }
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct HotKeys(Vec<(HotKey, KeyAssign)>);

impl Default for HotKeys {
    fn default() -> Self {
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

impl HotKeys {
    pub fn key_assign(&self, hotkey: HotKey) -> Option<&KeyAssign> {
        self.0.iter().find(|(h, _)| *h == hotkey).map(|(_, k)| k)
    }

    pub fn key_assign_mut(&mut self, hotkey: HotKey) -> Option<&mut KeyAssign> {
        self.0
            .iter_mut()
            .find(|(h, _)| *h == hotkey)
            .map(|(_, k)| k)
    }

    pub fn just_pressed(&self, hotkey: HotKey, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .find(|r| r.0 == hotkey)
            .map_or(false, |r| r.1.just_pressed(input_state))
    }

    pub fn pressed(&self, hotkey: HotKey, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .find(|r| r.0 == hotkey)
            .map_or(false, |r| r.1.pressed(input_state))
    }
}

pub struct IsTurbo(pub bool);

fn check_hotkey(
    config: Res<Config>,
    input_keycode: Res<Input<KeyCode>>,
    input_gamepad_button: Res<Input<GamepadButton>>,
    input_gamepad_axis: Res<Axis<GamepadAxis>>,
    mut writer: EventWriter<HotKey>,
    mut is_turbo: ResMut<IsTurbo>,
) {
    let input_state = InputState::new(&input_keycode, &input_gamepad_button, &input_gamepad_axis);

    for hotkey in all::<HotKey>() {
        if config.hotkeys().just_pressed(hotkey, &input_state) {
            writer.send(hotkey);
        }
    }

    is_turbo.0 = config.hotkeys().pressed(
        HotKey::Turbo,
        &InputState::new(&input_keycode, &input_gamepad_button, &input_gamepad_axis),
    );
}

fn process_hotkey(
    mut config: ResMut<Config>,
    mut reader: EventReader<HotKey>,
    mut app_state: ResMut<State<AppState>>,
    mut gb_state: Option<ResMut<GameBoyState>>,
    mut ui_state: ResMut<UiState>,
    mut window_control_event: EventWriter<WindowControlEvent>,
    mut message_event: EventWriter<ShowMessage>,
) {
    for hotkey in reader.iter() {
        match hotkey {
            HotKey::Reset => {
                if let Some(state) = &mut gb_state {
                    state.gb.reset();
                    message_event.send(ShowMessage("Reset machine".to_string()));
                }
            }
            HotKey::StateSave => {
                if let Some(state) = &mut gb_state {
                    state
                        .save_state(ui_state.state_save_slot, config.as_ref())
                        .unwrap();
                    message_event.send(ShowMessage(format!(
                        "State saved: #{}",
                        ui_state.state_save_slot
                    )));
                }
            }
            HotKey::StateLoad => {
                if let Some(state) = &mut gb_state {
                    if let Err(e) = state.load_state(ui_state.state_save_slot, config.as_ref()) {
                        message_event.send(ShowMessage("Failed to load state".to_string()));
                        error!("Failed to load state: {}", e);
                    } else {
                        message_event.send(ShowMessage(format!(
                            "State loaded: #{}",
                            ui_state.state_save_slot
                        )));
                    }
                }
            }
            HotKey::NextSlot => {
                ui_state.state_save_slot += 1;
                message_event.send(ShowMessage(format!(
                    "State slot changed: #{}",
                    ui_state.state_save_slot
                )));
            }
            HotKey::PrevSlot => {
                ui_state.state_save_slot = ui_state.state_save_slot.saturating_sub(1);
                message_event.send(ShowMessage(format!(
                    "State slot changed: #{}",
                    ui_state.state_save_slot
                )));
            }
            HotKey::Rewind => {
                if app_state.current() == &AppState::Running {
                    let gb_state = gb_state.as_mut().unwrap();

                    let saved_state = AutoSavedState {
                        data: gb_state.gb.save_state(),
                        thumbnail: make_color_correction(
                            gb_state.gb.model().is_cgb() && config.color_correction(),
                        )
                        .frame_buffer_to_image(gb_state.gb.frame_buffer()),
                    };

                    gb_state.auto_saved_states.push_back(saved_state);
                    if gb_state.auto_saved_states.len() > config.auto_state_save_limit() {
                        gb_state.auto_saved_states.pop_front();
                    }

                    app_state.push(AppState::Rewinding).unwrap();
                }
            }
            HotKey::Menu => {
                if app_state.current() == &AppState::Running {
                    app_state.set(AppState::Menu).unwrap();
                }
                if app_state.current() == &AppState::Menu && gb_state.is_some() {
                    app_state.set(AppState::Running).unwrap();
                }
            }
            HotKey::FullScreen => {
                window_control_event.send(WindowControlEvent::ToggleFullscreen);
            }
            HotKey::ScaleUp => {
                let cur = config.scaling();
                config.set_scaling(cur + 1);
                window_control_event.send(WindowControlEvent::Restore);
            }
            HotKey::ScaleDown => {
                let cur = config.scaling();
                config.set_scaling(max(1, cur - 1));
                window_control_event.send(WindowControlEvent::Restore);
            }

            HotKey::Turbo => {}
        }
    }
}
