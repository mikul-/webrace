//! Input representation and Quake-compatible angle handling.
//!
//! Matches Quake/Warfork: yaw/pitch are 16-bit fixed-point (0..65535), scaled
//! by `m_yaw`/`m_pitch` (both 0.022) and `sensitivity`. This keeps the
//! sensitivity model byte-compatible with Quake/Source.

#![allow(dead_code)]

/// 65536 fixed-point unit circle, like Quake's `ANGLE` macros.
pub const ANGLE_2_PI: f32 = 65536.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct Cmd {
    pub forward: i8,
    pub right: i8,
    pub up: i8,
    pub buttons: u16,
    pub yaw: u16,
    pub pitch: u16,
    pub roll: u16,
}

pub const BUTTON_ATTACK: u16 = 1;
pub const BUTTON_SPECIAL: u16 = 2; // dash / wall-jump in Warfork
pub const BUTTON_JUMP: u16 = 4;
pub const BUTTON_CROUCH: u16 = 8;

#[derive(Clone, Copy, Debug)]
pub struct MouseConfig {
    pub sensitivity: f32,
    pub m_yaw: f32,
    pub m_pitch: f32,
}

impl Default for MouseConfig {
    fn default() -> Self {
        MouseConfig {
            sensitivity: 1.72,
            m_yaw: 0.022,
            m_pitch: 0.022,
        }
    }
}

/// Accumulate relative mouse motion into floating angles, then quantize to
/// 16-bit fixed-point for the command (exact Quake behavior).
#[derive(Clone, Debug)]
pub struct Angles {
    pub yaw: f32,
    pub pitch: f32,
    pub yaw_frac: f32,
    pub pitch_frac: f32,
}

impl Default for Angles {
    fn default() -> Self {
        Angles { yaw: 0.0, pitch: 0.0, yaw_frac: 0.0, pitch_frac: 0.0 }
    }
}

impl Angles {
    pub fn add_mouse(&mut self, dx: f32, dy: f32, cfg: &MouseConfig) {
        // Warfork/Quake: yaw decreases with +dx (turn right), pitch decreases
        // with +dy (look down). Both scaled by sensitivity * m_*.
        self.yaw_frac -= dx * cfg.sensitivity * cfg.m_yaw;
        self.pitch_frac -= dy * cfg.sensitivity * cfg.m_pitch;
    }

    pub fn quantize(&mut self, out_yaw: &mut u16, out_pitch: &mut u16) {
        let full_yaw = (self.yaw_frac % ANGLE_2_PI + ANGLE_2_PI) % ANGLE_2_PI;
        let pitch = self.pitch_frac.clamp(-16384.0, 16384.0);
        self.yaw = full_yaw;
        self.pitch = pitch;
        // 16-bit wrap
        *out_yaw = (full_yaw as u16).wrapping_add(0);
        *out_pitch = ((pitch as i32).rem_euclid(65536)) as u16;
        self.yaw_frac = full_yaw;
    }

    pub fn to_yaw_rad(&self) -> f32 {
        (self.yaw - 32768.0) / 32768.0 * std::f32::consts::PI
    }
    pub fn to_pitch_rad(&self) -> f32 {
        self.pitch / 32768.0 * std::f32::consts::PI
    }
}
