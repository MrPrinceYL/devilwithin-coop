//! Controller state shared between the Bevy input system and the emulated controller ports
//! (DualShock 2: digital buttons, analog sticks).

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering::Relaxed};

// Button bits (active high here; padman reports them inverted), the PS2 order.
pub const SELECT: u16 = 1 << 0;
pub const L3: u16 = 1 << 1;
pub const R3: u16 = 1 << 2;
pub const START: u16 = 1 << 3;
pub const UP: u16 = 1 << 4;
pub const RIGHT: u16 = 1 << 5;
pub const DOWN: u16 = 1 << 6;
pub const LEFT: u16 = 1 << 7;
pub const L2: u16 = 1 << 8;
pub const R2: u16 = 1 << 9;
pub const L1: u16 = 1 << 10;
pub const R1: u16 = 1 << 11;
pub const TRIANGLE: u16 = 1 << 12;
pub const CIRCLE: u16 = 1 << 13;
pub const CROSS: u16 = 1 << 14;
pub const SQUARE: u16 = 1 << 15;

/// Port 1 reads as "nothing pressed, sticks centred" to the game while set (a front-end menu
/// that takes the pad, e.g. the Devil Within mod menu); `PadShared` keeps the real state.
pub static BLOCK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Left stick turned by this angle per port (0x10000 = full turn, counter-clockwise) before the
/// game reads it: co-op split screen, where each player steers relative to their own camera
/// while the game uses the camera it holds that frame (a mod hook, set by the game crate).
pub static TURN: [std::sync::atomic::AtomicI32; 2] = [std::sync::atomic::AtomicI32::new(0), std::sync::atomic::AtomicI32::new(0)];

/// `sticks` (RX, RY, LX, LY bytes) with the left stick turned by `turn` (see TURN).
pub fn turn_left_stick(sticks: u32, turn: i32) -> u32 {
    if turn == 0 || sticks == 0 {
        return sticks;
    }
    let x = ((sticks >> 16) & 0xFF) as f32 - 128.0;
    let y = 128.0 - ((sticks >> 24) & 0xFF) as f32;
    let a = turn as f32 * std::f32::consts::TAU / 65536.0;
    let (s, c) = a.sin_cos();
    let byte = |v: f32| (v.round() as i32).clamp(0, 255) as u32;
    let (nx, ny) = (byte(128.0 + x * c - y * s), byte(128.0 - (x * s + y * c)));
    (sticks & 0xFFFF) | nx << 16 | ny << 24
}

pub struct PadShared {
    pub buttons: [AtomicU16; 2],
    pub connected: [AtomicBool; 2],
    /// RX, RY, LX, LY bytes (0x80 = centre); 0 = all centred.
    pub sticks: [AtomicU32; 2],
}

impl Default for PadShared {
    /// Port 1 connected, port 2 empty, no buttons held.
    fn default() -> Self {
        PadShared {
            buttons: [AtomicU16::new(0), AtomicU16::new(0)],
            connected: [AtomicBool::new(true), AtomicBool::new(false)],
            sticks: [AtomicU32::new(0), AtomicU32::new(0)],
        }
    }
}

impl PadShared {
    pub fn set(&self, port: usize, buttons: u16, connected: bool) {
        self.buttons[port].store(buttons, Relaxed);
        self.connected[port].store(connected, Relaxed);
    }
    /// RX, RY, LX, LY bytes (0x80 = centre; 0 = all centred).
    pub fn set_sticks(&self, port: usize, sticks: u32) {
        self.sticks[port].store(sticks, Relaxed);
    }
    pub fn get(&self, port: usize) -> (u16, bool) {
        (self.buttons[port].load(Relaxed), self.connected[port].load(Relaxed))
    }
}
