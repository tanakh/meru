use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum KeyCode {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamepadButton(pub Gamepad, pub GamepadButtonType);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gamepad(pub usize);

#[derive(PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum GamepadButtonType {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamepadAxis(pub Gamepad, pub GamepadAxisType);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamepadAxisType {
    LeftStickX,
    LeftStickY,
    LeftZ,
    RightStickX,
    RightStickY,
    RightZ,
    DPadX,
    DPadY,
}

#[derive(PartialEq, Eq, Default, Clone, Debug, Serialize, Deserialize)]
pub struct KeyAssign(pub Vec<MultiKey>);

impl From<SingleKey> for KeyAssign {
    fn from(key: SingleKey) -> Self {
        KeyAssign(vec![MultiKey(vec![key])])
    }
}

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

impl Display for ToStringKey<&KeyCode> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Display for ToStringKey<&GamepadButton> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let GamepadButton(gamepad, button) = &self.0;
        write!(f, "Pad{}.{}", gamepad.0, ToStringKey(button))
    }
}

impl Display for ToStringKey<&GamepadButtonType> {
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

impl Display for ToStringKey<(&GamepadAxis, &GamepadAxisDir)> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (axis, dir) = &self.0;
        let GamepadAxis(gamepad, axis) = axis;
        let dir = match dir {
            GamepadAxisDir::Pos => "+",
            GamepadAxisDir::Neg => "-",
        };
        write!(f, "Pad{}.{}{dir}", gamepad.0, ToStringKey(axis))
    }
}

impl Display for ToStringKey<&GamepadAxisType> {
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
            SingleKey::KeyCode(kc) => write!(f, "{}", ToStringKey(kc)),
            SingleKey::GamepadButton(button) => write!(f, "{}", ToStringKey(button)),
            SingleKey::GamepadAxis(axis, dir) => write!(f, "{}", ToStringKey((axis, dir))),
        }
    }
}

pub trait InputState {
    fn pressed(&self, key: &SingleKey) -> bool;
    fn just_pressed(&self, key: &SingleKey) -> bool;
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
    pub fn pressed(&self, input_state: &impl InputState) -> bool {
        self.0
            .iter()
            .any(|multi_key| multi_key.pressed(input_state))
    }

    pub fn just_pressed(&self, input_state: &impl InputState) -> bool {
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
    fn pressed(&self, input_state: &impl InputState) -> bool {
        self.0
            .iter()
            .all(|single_key| input_state.pressed(single_key))
    }

    fn just_pressed(&self, input_state: &impl InputState) -> bool {
        // all key are pressed and some key is just pressed
        self.pressed(input_state)
            && self
                .0
                .iter()
                .any(|single_key| input_state.just_pressed(single_key))
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
        KeyAssign(vec![MultiKey(vec![
            $crate::key_assign::SingleKey::KeyCode($crate::key_assign::KeyCode::$code),
        ])])
    };
}
pub use keycode;

#[macro_export]
macro_rules! pad_button {
    ($id:literal, $button:ident) => {
        $crate::key_assign::KeyAssign(vec![$crate::key_assign::MultiKey(vec![
            $crate::key_assign::SingleKey::GamepadButton($crate::key_assign::GamepadButton(
                $crate::key_assign::Gamepad($id),
                $crate::key_assign::GamepadButtonType::$button,
            )),
        ])])
    };
}
pub use pad_button;
