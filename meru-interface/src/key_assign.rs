use bevy_input::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct KeyAssign(pub Vec<MultiKey>);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiKey(pub Vec<SingleKey>);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SingleKey {
    KeyCode(KeyCode),
    GamepadButton(GamepadButton),
    GamepadAxis(GamepadAxis, GamepadAxisDir),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamepadAxisDir {
    Pos,
    Neg,
}

pub struct ToStringKey<T>(pub T);

impl Display for ToStringKey<KeyCode> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Display for ToStringKey<GamepadButton> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let GamepadButton(gamepad, button) = &self.0;
        write!(f, "Pad{}.{}", gamepad.0, ToStringKey(*button))
    }
}

impl Display for ToStringKey<GamepadButtonType> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use GamepadButtonType::*;
        write!(
            f,
            "{}",
            match self.0 {
                South => "S",
                East => "E",
                North => "N",
                West => "W",
                C => "C",
                Z => "Z",
                LeftTrigger => "LB",
                LeftTrigger2 => "LT",
                RightTrigger => "RB",
                RightTrigger2 => "RT",
                Select => "Select",
                Start => "Start",
                Mode => "Mode",
                LeftThumb => "LS",
                RightThumb => "RS",
                DPadUp => "DPadUp",
                DPadDown => "DPadDown",
                DPadLeft => "DPadLeft",
                DPadRight => "DPadRight",
            }
        )
    }
}

impl Display for ToStringKey<(GamepadAxis, GamepadAxisDir)> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (axis, dir) = &self.0;
        let GamepadAxis(gamepad, axis) = axis;
        let dir = match dir {
            GamepadAxisDir::Pos => "+",
            GamepadAxisDir::Neg => "-",
        };
        write!(f, "Pad{}.{}{dir}", gamepad.0, ToStringKey(*axis))
    }
}

impl Display for ToStringKey<GamepadAxisType> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use GamepadAxisType::*;
        write!(
            f,
            "{}",
            match self.0 {
                LeftStickX => "LX",
                LeftStickY => "LY",
                LeftZ => "LZ",
                RightStickX => "RX",
                RightStickY => "RY",
                RightZ => "RZ",
                DPadX => "DPadX",
                DPadY => "DPadY",
            }
        )
    }
}

impl Display for MultiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for single_key in &self.0 {
            if !first {
                write!(f, "+")?;
            }
            write!(f, "{}", single_key)?;
            first = false;
        }
        Ok(())
    }
}

impl Display for SingleKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SingleKey::KeyCode(kc) => write!(f, "{}", ToStringKey(*kc)),
            SingleKey::GamepadButton(button) => write!(f, "{}", ToStringKey(*button)),
            SingleKey::GamepadAxis(axis, dir) => write!(f, "{}", ToStringKey((*axis, *dir))),
        }
    }
}

impl KeyAssign {
    pub fn and(self, rhs: Self) -> Self {
        let mut ret = vec![];
        for l in self.0.into_iter() {
            for r in rhs.0.iter() {
                let mut t = l.0.clone();
                t.append(&mut r.0.clone());
                ret.push(MultiKey(t));
            }
        }
        Self(ret)
    }

    pub fn or(mut self, mut rhs: Self) -> Self {
        self.0.append(&mut rhs.0);
        self
    }

    pub fn pressed(&self, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .any(|multi_key| multi_key.pressed(input_state))
    }

    pub fn just_pressed(&self, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .any(|multi_key| multi_key.just_pressed(input_state))
    }

    pub fn extract_keycode(&self) -> Option<KeyCode> {
        for MultiKey(mk) in &self.0 {
            if let [SingleKey::KeyCode(r)] = &mk[..] {
                return Some(*r);
            }
        }
        None
    }

    pub fn insert_keycode(&mut self, kc: KeyCode) {
        for MultiKey(mk) in self.0.iter_mut() {
            if let [SingleKey::KeyCode(r)] = &mut mk[..] {
                *r = kc;
                return;
            }
        }
        self.0.push(MultiKey(vec![SingleKey::KeyCode(kc)]));
    }

    pub fn extract_gamepad(&self) -> Option<GamepadButton> {
        for MultiKey(mk) in &self.0 {
            if let [SingleKey::GamepadButton(r)] = &mk[..] {
                return Some(*r);
            }
        }
        None
    }

    pub fn insert_gamepad(&mut self, button: GamepadButton) {
        for MultiKey(mk) in self.0.iter_mut() {
            if let [SingleKey::GamepadButton(r)] = &mut mk[..] {
                *r = button;
                return;
            }
        }
        self.0
            .push(MultiKey(vec![SingleKey::GamepadButton(button)]));
    }
}

impl MultiKey {
    fn pressed(&self, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .all(|single_key| single_key.pressed(input_state))
    }

    fn just_pressed(&self, input_state: &InputState<'_>) -> bool {
        // all key are pressed and some key is just pressed
        self.pressed(input_state)
            && self
                .0
                .iter()
                .any(|single_key| single_key.just_pressed(input_state))
    }
}

impl SingleKey {
    fn pressed(&self, input_state: &InputState<'_>) -> bool {
        match self {
            SingleKey::KeyCode(keycode) => input_state.input_keycode.pressed(*keycode),
            SingleKey::GamepadButton(button) => input_state.input_gamepad_button.pressed(*button),
            SingleKey::GamepadAxis(axis, dir) => {
                input_state
                    .input_gamepad_axis
                    .get(*axis)
                    .map_or(false, |r| match dir {
                        GamepadAxisDir::Pos => r >= 0.5,
                        GamepadAxisDir::Neg => r <= -0.5,
                    })
            }
        }
    }

    fn just_pressed(&self, input_state: &InputState<'_>) -> bool {
        match self {
            SingleKey::KeyCode(keycode) => input_state.input_keycode.just_pressed(*keycode),
            SingleKey::GamepadButton(button) => {
                input_state.input_gamepad_button.just_pressed(*button)
            }
            SingleKey::GamepadAxis(_axis, _dir) => {
                // TODO
                false
            }
        }
    }
}

#[macro_export]
macro_rules! any {
    ($x:expr, $($xs:expr),* $(,)?) => {
        [$($xs),*].into_iter().fold($x, |a, b| a.or(b))
    };
}
pub use any;

#[macro_export]
macro_rules! all {
    ($x:expr, $($xs:expr),* $(,)?) => {{
        [$($xs),*].into_iter().fold($x, |a, b| a.and(b))
    }};
}
pub use all;

#[macro_export]
macro_rules! keycode {
    ($code:ident) => {
        KeyAssign(vec![MultiKey(vec![SingleKey::KeyCode(KeyCode::$code)])])
    };
}
pub use keycode;

#[macro_export]
macro_rules! pad_button {
    ($id:literal, $button:ident) => {
        KeyAssign(vec![MultiKey(vec![SingleKey::GamepadButton(
            GamepadButton(Gamepad($id), GamepadButtonType::$button),
        )])])
    };
}
pub use pad_button;

pub struct InputState<'a> {
    input_keycode: &'a Input<KeyCode>,
    input_gamepad_button: &'a Input<GamepadButton>,
    input_gamepad_axis: &'a Axis<GamepadAxis>,
}

impl<'a> InputState<'a> {
    pub fn new(
        input_keycode: &'a Input<KeyCode>,
        input_gamepad_button: &'a Input<GamepadButton>,
        input_gamepad_axis: &'a Axis<GamepadAxis>,
    ) -> Self {
        Self {
            input_keycode,
            input_gamepad_button,
            input_gamepad_axis,
        }
    }
}
