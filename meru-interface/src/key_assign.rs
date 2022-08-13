use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum KeyCode {
    Key1 = b'1' as isize,
    Key2 = b'2' as isize,
    Key3 = b'3' as isize,
    Key4 = b'4' as isize,
    Key5 = b'5' as isize,
    Key6 = b'6' as isize,
    Key7 = b'7' as isize,
    Key8 = b'8' as isize,
    Key9 = b'9' as isize,
    Key0 = b'0' as isize,

    A = b'A' as isize,
    B = b'B' as isize,
    C = b'C' as isize,
    D = b'D' as isize,
    E = b'E' as isize,
    F = b'F' as isize,
    G = b'G' as isize,
    H = b'H' as isize,
    I = b'I' as isize,
    J = b'J' as isize,
    K = b'K' as isize,
    L = b'L' as isize,
    M = b'M' as isize,
    N = b'N' as isize,
    O = b'O' as isize,
    P = b'P' as isize,
    Q = b'Q' as isize,
    R = b'R' as isize,
    S = b'S' as isize,
    T = b'T' as isize,
    U = b'U' as isize,
    V = b'V' as isize,
    W = b'W' as isize,
    X = b'X' as isize,
    Y = b'Y' as isize,
    Z = b'Z' as isize,

    Escape = b'\x1B' as isize,
    F1 = 112 as isize,
    F2 = 113 as isize,
    F3 = 114 as isize,
    F4 = 115 as isize,
    F5 = 116 as isize,
    F6 = 117 as isize,
    F7 = 118 as isize,
    F8 = 119 as isize,
    F9 = 120 as isize,
    F10 = 121 as isize,
    F11 = 122 as isize,
    F12 = 123 as isize,

    // Snapshot,
    Scroll = 145 as isize,
    Pause = 19 as isize,
    Insert = 45 as isize,
    Home = 36 as isize,
    Delete = 46 as isize,
    End = 35 as isize,
    PageDown = 34 as isize,
    PageUp = 33 as isize,
    Left = 37 as isize,
    Up = 38 as isize,
    Right = 39 as isize,
    Down = 40 as isize,
    Backspace = b'\x08' as isize,
    Return = b'\n' as isize,
    Space = b' ' as isize,
    // Compose,
    // Caret,
    Numlock = 144 as isize,

    Numpad0 = 96 as isize,
    Numpad1 = 97 as isize,
    Numpad2 = 98 as isize,
    Numpad3 = 99 as isize,
    Numpad4 = 100 as isize,
    Numpad5 = 101 as isize,
    Numpad6 = 102 as isize,
    Numpad7 = 103 as isize,
    Numpad8 = 104 as isize,
    Numpad9 = 105 as isize,
    // AbntC1,
    // AbntC2,
    NumpadAdd = 107 as isize,

    // Apostrophe,
    // Apps,
    Asterisk = b'*' as isize,
    Plus = b'+' as isize,
    At = 192 as isize,
    // Ax,
    Backslash = b'\\' as isize,
    // Calculator,
    // Capital,
    Colon = 186 as isize,
    Comma = 188 as isize,
    // Convert,
    // NumpadDecimal,
    // NumpadDivide,
    Equals = b'=' as isize,
    // Grave,
    Kana = b'\x15' as isize,
    // Kanji,
    LAlt = 18 as isize,
    LBracket = 219 as isize,
    LControl = 17 as isize,
    LShift = 16 as isize,
    LWin = 91 as isize,

    // Mail,
    // MediaSelect,
    // MediaStop,
    Minus = 189 as isize,
    NumpadMultiply = 106 as isize,
    // Mute,
    // MyComputer,
    // NavigateForward,
    // NavigateBackward,
    // NextTrack,
    // NoConvert,
    // NumpadComma,
    // NumpadEnter,
    // NumpadEquals,
    // Oem102,
    Period = 190 as isize,
    // PlayPause,
    // Power,
    // PrevTrack,
    // RAlt,
    RBracket = 221 as isize,
    // RControl,
    // RShift,
    // RWin,
    Semicolon = 187 as isize,
    Slash = 191 as isize,
    // Sleep,
    // Stop,
    NumpadSubtract = 109 as isize,
    // Sysrq,
    Tab = b'\t' as isize,
    Underline = 226 as isize,
    // Unlabeled,
    // VolumeDown,
    // VolumeUp,
    // Wake,
    // WebBack,
    // WebFavorites,
    // WebForward,
    // WebHome,
    // WebRefresh,
    // WebSearch,
    // WebStop,
    Yen = 220 as isize,
    // Copy,
    // Paste,
    // Cut,
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
