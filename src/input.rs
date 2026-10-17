//! `JDInput` replacement: keyboard (DirectInput scan codes) and the first
//! joystick, sampled into "current" and "previous" snapshots so that
//! `pressed` means "went down since the previous `update`".

use macroquad::input::{KeyCode, is_key_down};

/// DirectInput key codes used by the game.
pub mod dik {
    pub const ESCAPE: u8 = 0x01;
    pub const RETURN: u8 = 0x1c;
    pub const LCONTROL: u8 = 0x1d;
    pub const LSHIFT: u8 = 0x2a;
    pub const Z: u8 = 0x2c;
    pub const X: u8 = 0x2d;
    pub const RSHIFT: u8 = 0x36;
    pub const NUMPADENTER: u8 = 0x9c;
    pub const RCONTROL: u8 = 0x9d;
    pub const UP: u8 = 0xc8;
    pub const LEFT: u8 = 0xcb;
    pub const RIGHT: u8 = 0xcd;
    pub const DOWN: u8 = 0xd0;
}

const KEYMAP: [(u8, KeyCode); 13] = [
    (dik::ESCAPE, KeyCode::Escape),
    (dik::RETURN, KeyCode::Enter),
    (dik::LCONTROL, KeyCode::LeftControl),
    (dik::LSHIFT, KeyCode::LeftShift),
    (dik::Z, KeyCode::Z),
    (dik::X, KeyCode::X),
    (dik::RSHIFT, KeyCode::RightShift),
    (dik::NUMPADENTER, KeyCode::KpEnter),
    (dik::RCONTROL, KeyCode::RightControl),
    (dik::UP, KeyCode::Up),
    (dik::LEFT, KeyCode::Left),
    (dik::RIGHT, KeyCode::Right),
    (dik::DOWN, KeyCode::Down),
];

#[derive(Clone, Copy)]
struct Snapshot {
    keys: [bool; 256],
    /// up, right, down, left
    joy_dir: [bool; 4],
    joy_btn: [bool; 8],
}

impl Default for Snapshot {
    fn default() -> Self {
        Snapshot {
            keys: [false; 256],
            joy_dir: [false; 4],
            joy_btn: [false; 8],
        }
    }
}

impl Default for Input {
    fn default() -> Self {
        Input::new()
    }
}

pub struct Input {
    cur: Snapshot,
    prev: Snapshot,
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    /// Synthetic snapshot used by the automated smoke test.
    pub scripted: Option<Box<dyn FnMut(u64) -> Vec<u8>>>,
    frame: u64,
}

impl Input {
    pub fn new() -> Input {
        Input {
            cur: Snapshot {
                keys: [false; 256],
                ..Default::default()
            },
            prev: Snapshot {
                keys: [false; 256],
                ..Default::default()
            },
            #[cfg(feature = "gamepad")]
            gilrs: gilrs::Gilrs::new().ok(),
            scripted: None,
            frame: 0,
        }
    }

    /// Drains gamepad events; call every frame.
    pub fn poll(&mut self) {
        #[cfg(feature = "gamepad")]
        if let Some(g) = &mut self.gilrs {
            while g.next_event().is_some() {}
        }
    }

    fn read(&mut self) -> Snapshot {
        let mut s = Snapshot {
            keys: [false; 256],
            ..Default::default()
        };
        if let Some(script) = &mut self.scripted {
            for k in script(self.frame) {
                s.keys[k as usize] = true;
            }
            return s;
        }
        for (code, key) in KEYMAP {
            if is_key_down(key) {
                s.keys[code as usize] = true;
            }
        }
        #[cfg(feature = "gamepad")]
        if let Some(g) = &self.gilrs {
            use gilrs::{Axis, Button};
            if let Some((_, pad)) = g.gamepads().next() {
                let ax = pad.value(Axis::LeftStickX);
                let ay = pad.value(Axis::LeftStickY);
                s.joy_dir[0] = pad.is_pressed(Button::DPadUp) || ay > 0.5;
                s.joy_dir[1] = pad.is_pressed(Button::DPadRight) || ax > 0.5;
                s.joy_dir[2] = pad.is_pressed(Button::DPadDown) || ay < -0.5;
                s.joy_dir[3] = pad.is_pressed(Button::DPadLeft) || ax < -0.5;
                s.joy_btn[0] = pad.is_pressed(Button::South) || pad.is_pressed(Button::West);
                s.joy_btn[1] = pad.is_pressed(Button::East)
                    || pad.is_pressed(Button::North)
                    || pad.is_pressed(Button::LeftTrigger)
                    || pad.is_pressed(Button::RightTrigger);
                s.joy_btn[2] = pad.is_pressed(Button::Start);
                s.joy_btn[3] = pad.is_pressed(Button::Select);
            }
        }
        s
    }

    /// `JDInput::UpdateInputState`.
    pub fn update(&mut self) {
        self.prev = self.cur;
        self.cur = self.read();
        self.frame += 1;
    }

    pub fn key(&self, k: u8) -> bool {
        self.cur.keys[k as usize]
    }

    pub fn key_pressed(&self, k: u8) -> bool {
        self.cur.keys[k as usize] && !self.prev.keys[k as usize]
    }

    pub fn joy_dir(&self, d: usize) -> bool {
        self.cur.joy_dir[d]
    }

    pub fn joy_dir_pressed(&self, d: usize) -> bool {
        self.cur.joy_dir[d] && !self.prev.joy_dir[d]
    }

    pub fn joy_button(&self, b: usize) -> bool {
        self.cur.joy_btn[b]
    }

    pub fn joy_button_pressed(&self, b: usize) -> bool {
        self.cur.joy_btn[b] && !self.prev.joy_btn[b]
    }

    /// Gamepad Start/Back act as Escape (there is no keyboard on a pad).
    pub fn escape_pressed(&self) -> bool {
        self.key_pressed(dik::ESCAPE) || self.joy_button_pressed(3)
    }

    /// `CheckButtonPressed`: any confirm key or joystick button 0/1.
    pub fn button_pressed(&self) -> bool {
        [
            dik::NUMPADENTER,
            dik::RETURN,
            dik::LSHIFT,
            dik::RSHIFT,
            dik::LCONTROL,
            dik::RCONTROL,
            dik::Z,
            dik::X,
        ]
        .iter()
        .any(|&k| self.key_pressed(k))
            || self.joy_button_pressed(0)
            || self.joy_button_pressed(1)
            || self.joy_button_pressed(2)
    }
}
