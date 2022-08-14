use bevy::prelude::*;

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

impl<'a> meru_interface::InputState for InputState<'a> {
    fn pressed(&self, key: &meru_interface::SingleKey) -> bool {
        use meru_interface::SingleKey;
        match key {
            SingleKey::KeyCode(key_code) => self.input_keycode.pressed(to_bevy_keycode(key_code)),
            SingleKey::GamepadButton(button) => self
                .input_gamepad_button
                .pressed(to_bevy_gamepad_button(button)),
            SingleKey::GamepadAxis(_, _) => todo!(),
        }
    }

    fn just_pressed(&self, key: &meru_interface::key_assign::SingleKey) -> bool {
        use meru_interface::SingleKey;
        match key {
            SingleKey::KeyCode(key_code) => {
                self.input_keycode.just_pressed(to_bevy_keycode(key_code))
            }
            SingleKey::GamepadButton(button) => self
                .input_gamepad_button
                .just_pressed(to_bevy_gamepad_button(button)),
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

pub fn to_bevy_keycode(key_code: &meru_interface::KeyCode) -> bevy::prelude::KeyCode {
    map_code!(key_code, meru_interface::KeyCode, bevy::prelude::KeyCode)
}

pub fn to_meru_keycode(key_code: &bevy::prelude::KeyCode) -> meru_interface::KeyCode {
    map_code!(key_code, bevy::prelude::KeyCode, meru_interface::KeyCode)
}

pub fn to_bevy_gamepad_button(
    button: &meru_interface::GamepadButton,
) -> bevy::prelude::GamepadButton {
    bevy::prelude::GamepadButton(
        bevy::prelude::Gamepad(button.0 .0),
        to_bevy_gamepad_button_type(&button.1),
    )
}

pub fn to_meru_gamepad_button(
    button: &bevy::prelude::GamepadButton,
) -> meru_interface::GamepadButton {
    meru_interface::GamepadButton(
        meru_interface::Gamepad(button.0 .0),
        to_meru_gamepad_button_type(&button.1),
    )
}

pub fn to_bevy_gamepad_button_type(
    button_type: &meru_interface::GamepadButtonType,
) -> bevy::prelude::GamepadButtonType {
    map_gamepad_button_type!(
        button_type,
        meru_interface::GamepadButtonType,
        bevy::prelude::GamepadButtonType
    )
}

pub fn to_meru_gamepad_button_type(
    button_type: &bevy::prelude::GamepadButtonType,
) -> meru_interface::GamepadButtonType {
    map_gamepad_button_type!(
        button_type,
        bevy::prelude::GamepadButtonType,
        meru_interface::GamepadButtonType
    )
}
