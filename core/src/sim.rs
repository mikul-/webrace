//! High-level simulation owned by the browser: world + player + camera.
//!
//! Exposed over wasm-bindgen as the "session" the client drives each frame:
//! feed mouse deltas + held keys, step physics, read the resulting camera.

use crate::bsp::Bsp;
use crate::input::{Angles, Cmd, MouseConfig, BUTTON_CROUCH, BUTTON_JUMP, BUTTON_SPECIAL};
use crate::pmove::{Pmove, PlayerState};
use crate::trace::World;

/// A playable session bound to one loaded map.
pub struct Session {
    pmove: Pmove,
    ps: PlayerState,
    angles: Angles,
    mouse: MouseConfig,
    held_cmd: Cmd,
}

impl Session {
    pub fn new(bsp: &Bsp, spawn_index: usize) -> Result<Session, String> {
        let world = World::from_bsp(bsp);
        if world.brush_plane_offsets.is_empty() {
            return Err("map has no collision brushes".into());
        }

        let frametime = 1.0 / crate::TICK_RATE as f32;
        let mut ps = PlayerState::default();

        // Place at a spawn point (or origin if none).
        if let Some(sp) = bsp.spawns.get(spawn_index) {
            ps.origin = sp.origin;
            ps.viewangles = [0.0, sp.yaw, 0.0];
        }

        // Drop onto the ground.
        let mut pmove = Pmove::new(world, frametime);
        pmove.drop_to_ground(&mut ps);

        let yaw = ps.viewangles[1];

        Ok(Session {
            pmove,
            ps,
            angles: Angles { yaw, ..Default::default() },
            mouse: MouseConfig {
                sensitivity: 1.72,
                m_yaw: 0.022,
                m_pitch: 0.022,
            },
            held_cmd: Cmd::default(),
        })
    }

    pub fn set_sensitivity(&mut self, s: f32) {
        self.mouse.sensitivity = s;
    }

    /// Add relative mouse motion (raw input, `movementX`/`movementY`).
    pub fn add_mouse(&mut self, dx: f32, dy: f32) {
        self.angles.add_mouse(dx, dy, &self.mouse);
    }

    /// Advance one tick given held inputs.
    pub fn step(&mut self) {
        let mut cmd = self.held_cmd;

        // Quantize angles into the command.
        self.angles.quantize(&mut cmd.yaw, &mut cmd.pitch);
        // Store view angles in the player state for forward/right calc.
        self.ps.viewangles = [
            self.angles.to_pitch_rad(),
            self.angles.to_yaw_rad(),
            0.0,
        ];

        self.pmove.step(&mut self.ps, &cmd);
    }

    /// Held-button state, set by the JS side each frame.
    pub fn set_keys(&mut self, forward: bool, back: bool, left: bool, right: bool, jump: bool, crouch: bool, special: bool, attack: bool) {
        let mut c = Cmd::default();
        // Move values are -127..127 (Quake usercmd range).
        if forward { c.forward += 127; }
        if back { c.forward -= 127; }
        if right { c.right += 127; }
        if left { c.right -= 127; }
        if jump { c.up = 127; c.buttons |= BUTTON_JUMP; }
        if crouch { c.buttons |= BUTTON_CROUCH; }
        if special { c.buttons |= BUTTON_SPECIAL; }
        if attack { c.buttons |= crate::input::BUTTON_ATTACK; }
        self.held_cmd = c;
    }

    /// Camera eye position (render space). The player origin is the box-center
    /// basis (feet at origin[2]-24); the eye sits ~26 units above that.
    pub fn eye(&self) -> [f32; 3] {
        [self.ps.origin[0], self.ps.origin[1], self.ps.origin[2] + 26.0]
    }

    pub fn yaw(&self) -> f32 { self.ps.viewangles[1] }
    pub fn pitch(&self) -> f32 { self.ps.viewangles[0] }
    pub fn speed(&self) -> f32 { self.ps.speed }
    pub fn on_ground(&self) -> bool { self.ps.on_ground }
    pub fn origin(&self) -> [f32; 3] { self.ps.origin }
    pub fn velocity(&self) -> [f32; 3] { self.ps.velocity }
}
