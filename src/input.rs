use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use tgbr_core::{Input as GameBoyInput, Pad};

use crate::{config::Config, key_assign::*};

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyConfig {
    pub up: KeyAssign,
    pub down: KeyAssign,
    pub left: KeyAssign,
    pub right: KeyAssign,
    pub a: KeyAssign,
    pub b: KeyAssign,
    pub start: KeyAssign,
    pub select: KeyAssign,
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            up: any!(keycode!(Up), pad_button!(0, DPadUp)),
            down: any!(keycode!(Down), pad_button!(0, DPadDown)),
            left: any!(keycode!(Left), pad_button!(0, DPadLeft)),
            right: any!(keycode!(Right), pad_button!(0, DPadRight)),
            a: any!(keycode!(X), pad_button!(0, South)),
            b: any!(keycode!(Z), pad_button!(0, West)),
            start: any!(keycode!(Return), pad_button!(0, Start)),
            select: any!(keycode!(RShift), pad_button!(0, Select)),
        }
    }
}

impl KeyConfig {
    fn input(&self, input_state: &InputState) -> GameBoyInput {
        GameBoyInput {
            pad: Pad {
                up: self.up.pressed(input_state),
                down: self.down.pressed(input_state),
                left: self.left.pressed(input_state),
                right: self.right.pressed(input_state),
                a: self.a.pressed(input_state),
                b: self.b.pressed(input_state),
                start: self.start.pressed(input_state),
                select: self.select.pressed(input_state),
            },
        }
    }
}

pub fn gameboy_input_system(
    config: Res<Config>,
    input_keycode: Res<Input<KeyCode>>,
    input_gamepad_button: Res<Input<GamepadButton>>,
    input_gamepad_axis: Res<Axis<GamepadAxis>>,
    mut input: ResMut<GameBoyInput>,
) {
    *input = config.key_config().input(&InputState::new(
        &input_keycode,
        &input_gamepad_button,
        &input_gamepad_axis,
    ));
}
