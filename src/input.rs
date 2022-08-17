use bevy::prelude::*;
use meru_interface::KeyAssign;
use serde::{Deserialize, Serialize};

pub struct InputState<'a> {
    keycode: &'a Input<KeyCode>,
    gamepad_button: &'a Input<GamepadButton>,
    gamepad_axis: &'a Axis<GamepadAxis>,
}

impl<'a> InputState<'a> {
    pub fn new(
        input_keycode: &'a Input<KeyCode>,
        input_gamepad_button: &'a Input<GamepadButton>,
        input_gamepad_axis: &'a Axis<GamepadAxis>,
    ) -> Self {
        Self {
            keycode: input_keycode,
            gamepad_button: input_gamepad_button,
            gamepad_axis: input_gamepad_axis,
        }
    }
}

impl<'a> meru_interface::InputState for InputState<'a> {
    fn pressed(&self, key: &meru_interface::SingleKey) -> bool {
        use meru_interface::SingleKey;
        match key {
            SingleKey::KeyCode(key_code) => self.keycode.pressed(ConvertInput(*key_code).into()),
            SingleKey::GamepadButton(button) => {
                self.gamepad_button.pressed(ConvertInput(*button).into())
            }
            SingleKey::GamepadAxis(axis, dir) => {
                let value = self
                    .gamepad_axis
                    .get(ConvertInput(*axis).into())
                    .unwrap_or(0.0);
                match dir {
                    meru_interface::key_assign::GamepadAxisDir::Pos => {
                        value > bevy::input::Axis::<GamepadAxis>::MAX / 2.0
                    }
                    meru_interface::key_assign::GamepadAxisDir::Neg => {
                        value < bevy::input::Axis::<GamepadAxis>::MIN / 2.0
                    }
                }
            }
        }
    }

    fn just_pressed(&self, key: &meru_interface::key_assign::SingleKey) -> bool {
        use meru_interface::SingleKey;
        match key {
            SingleKey::KeyCode(key_code) => {
                self.keycode.just_pressed(ConvertInput(*key_code).into())
            }
            SingleKey::GamepadButton(button) => self
                .gamepad_button
                .just_pressed(ConvertInput(*button).into()),
            SingleKey::GamepadAxis(_, _) => todo!(),
        }
    }
}

macro_rules! map_macro {
    ($macro_name:ident, $($key:ident),* $(,)?) => {
        macro_rules! $macro_name {
            ($key_code:expr, $from:ty, $to:ty) => {
                match $key_code {
                    $(
                        <$from>::$key => <$to>::$key,
                    )*
                }
            }
        }
    }
}

map_macro! {
    map_code,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    Snapshot,
    Scroll,
    Pause,
    Insert,
    Home,
    Delete,
    End,
    PageDown,
    PageUp,
    Left,
    Up,
    Right,
    Down,
    Back,
    Return,
    Space,
    Compose,
    Caret,
    Numlock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    AbntC1,
    AbntC2,
    NumpadAdd,
    Apostrophe,
    Apps,
    Asterisk,
    Plus,
    At,
    Ax,
    Backslash,
    Calculator,
    Capital,
    Colon,
    Comma,
    Convert,
    NumpadDecimal,
    NumpadDivide,
    Equals,
    Grave,
    Kana,
    Kanji,
    LAlt,
    LBracket,
    LControl,
    LShift,
    LWin,
    Mail,
    MediaSelect,
    MediaStop,
    Minus,
    NumpadMultiply,
    Mute,
    MyComputer,
    NavigateForward,
    NavigateBackward,
    NextTrack,
    NoConvert,
    NumpadComma,
    NumpadEnter,
    NumpadEquals,
    Oem102,
    Period,
    PlayPause,
    Power,
    PrevTrack,
    RAlt,
    RBracket,
    RControl,
    RShift,
    RWin,
    Semicolon,
    Slash,
    Sleep,
    Stop,
    NumpadSubtract,
    Sysrq,
    Tab,
    Underline,
    Unlabeled,
    VolumeDown,
    VolumeUp,
    Wake,
    WebBack,
    WebFavorites,
    WebForward,
    WebHome,
    WebRefresh,
    WebSearch,
    WebStop,
    Yen,
    Copy,
    Paste,
    Cut,
}

map_macro! {
    map_gamepad_button_type,
    South,
    East,
    North,
    West,
    C,
    Z,
    LeftTrigger,
    LeftTrigger2,
    RightTrigger,
    RightTrigger2,
    Select,
    Start,
    Mode,
    LeftThumb,
    RightThumb,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

map_macro! {
    map_gamepad_axis_type,
    LeftStickX,
    LeftStickY,
    LeftZ,
    RightStickX,
    RightStickY,
    RightZ,
}

pub struct ConvertInput<T>(pub T);

impl From<ConvertInput<meru_interface::KeyCode>> for bevy::prelude::KeyCode {
    fn from(key_code: ConvertInput<meru_interface::KeyCode>) -> Self {
        map_code!(key_code.0, meru_interface::KeyCode, bevy::prelude::KeyCode)
    }
}

impl From<ConvertInput<bevy::prelude::KeyCode>> for meru_interface::KeyCode {
    fn from(key_code: ConvertInput<bevy::prelude::KeyCode>) -> Self {
        map_code!(key_code.0, bevy::prelude::KeyCode, meru_interface::KeyCode)
    }
}

impl From<ConvertInput<meru_interface::GamepadButton>> for bevy::prelude::GamepadButton {
    fn from(button: ConvertInput<meru_interface::GamepadButton>) -> Self {
        bevy::prelude::GamepadButton::new(
            bevy::prelude::Gamepad::new(button.0.gamepad.id),
            ConvertInput(button.0.button_type).into(),
        )
    }
}

impl From<ConvertInput<bevy::prelude::GamepadButton>> for meru_interface::GamepadButton {
    fn from(button: ConvertInput<bevy::input::gamepad::GamepadButton>) -> Self {
        meru_interface::GamepadButton::new(
            meru_interface::Gamepad::new(button.0.gamepad.id),
            ConvertInput(button.0.button_type).into(),
        )
    }
}

impl From<ConvertInput<meru_interface::GamepadButtonType>> for bevy::prelude::GamepadButtonType {
    fn from(button_type: ConvertInput<meru_interface::GamepadButtonType>) -> Self {
        map_gamepad_button_type!(
            button_type.0,
            meru_interface::GamepadButtonType,
            bevy::prelude::GamepadButtonType
        )
    }
}

impl From<ConvertInput<bevy::prelude::GamepadButtonType>> for meru_interface::GamepadButtonType {
    fn from(button_type: ConvertInput<bevy::prelude::GamepadButtonType>) -> Self {
        map_gamepad_button_type!(
            button_type.0,
            bevy::prelude::GamepadButtonType,
            meru_interface::GamepadButtonType
        )
    }
}

impl From<ConvertInput<meru_interface::GamepadAxis>> for bevy::prelude::GamepadAxis {
    fn from(axis: ConvertInput<meru_interface::GamepadAxis>) -> Self {
        bevy::prelude::GamepadAxis::new(
            bevy::prelude::Gamepad::new(axis.0.gamepad.id),
            ConvertInput(axis.0.axis_type).into(),
        )
    }
}

impl From<ConvertInput<bevy::prelude::GamepadAxis>> for meru_interface::GamepadAxis {
    fn from(axis: ConvertInput<bevy::prelude::GamepadAxis>) -> Self {
        meru_interface::GamepadAxis::new(
            meru_interface::Gamepad::new(axis.0.gamepad.id),
            ConvertInput(axis.0.axis_type).into(),
        )
    }
}

impl From<ConvertInput<meru_interface::GamepadAxisType>> for bevy::prelude::GamepadAxisType {
    fn from(axis_type: ConvertInput<meru_interface::GamepadAxisType>) -> Self {
        map_gamepad_axis_type!(
            axis_type.0,
            meru_interface::GamepadAxisType,
            bevy::prelude::GamepadAxisType
        )
    }
}

impl From<ConvertInput<bevy::prelude::GamepadAxisType>> for meru_interface::GamepadAxisType {
    fn from(axis_type: ConvertInput<bevy::prelude::GamepadAxisType>) -> Self {
        map_gamepad_axis_type!(
            axis_type.0,
            bevy::prelude::GamepadAxisType,
            meru_interface::GamepadAxisType
        )
    }
}

#[derive(PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct KeyConfig<Key>(pub Vec<(Key, KeyAssign)>);

impl<Key: PartialEq + Clone> KeyConfig<Key> {
    pub fn key_assign(&self, key: &Key) -> Option<&KeyAssign> {
        self.0.iter().find(|(h, _)| h == key).map(|(_, k)| k)
    }

    pub fn key_assign_mut(&mut self, key: &Key) -> Option<&mut KeyAssign> {
        self.0.iter_mut().find(|(h, _)| h == key).map(|(_, k)| k)
    }

    pub fn insert_keycode(&mut self, key: &Key, key_code: meru_interface::KeyCode) {
        if let Some(key_assign) = self.key_assign_mut(key) {
            key_assign.insert_keycode(key_code);
        } else {
            use meru_interface::key_assign::*;
            self.0
                .push((key.clone(), SingleKey::KeyCode(key_code).into()));
        }
    }

    pub fn insert_gamepad(&mut self, key: &Key, button: meru_interface::GamepadButton) {
        if let Some(key_assign) = self.key_assign_mut(key) {
            key_assign.insert_gamepad(button);
        } else {
            use meru_interface::key_assign::*;
            self.0
                .push((key.clone(), SingleKey::GamepadButton(button).into()));
        }
    }

    pub fn just_pressed(&self, key: &Key, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .find(|r| &r.0 == key)
            .map_or(false, |r| r.1.just_pressed(input_state))
    }

    pub fn pressed(&self, key: &Key, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .find(|r| &r.0 == key)
            .map_or(false, |r| r.1.pressed(input_state))
    }
}
