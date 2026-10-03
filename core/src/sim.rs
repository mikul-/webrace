//! High-level simulation owned by the browser: world + player + camera.
//!
//! Exposed over wasm-bindgen as the "session" the client drives each frame:
//! feed mouse deltas + held keys, step physics, read the resulting camera.

use crate::bsp::{Bsp, RaceGate, RaceGateKind};
use crate::input::{Angles, Cmd, MouseConfig, BUTTON_CROUCH, BUTTON_JUMP, BUTTON_SPECIAL};
use crate::pmove::{Pmove, PlayerState};
use crate::trace::World;

/// Race progress: elapsed (ticks + current splits) between start and finish.
#[derive(Clone, Debug)]
pub struct Race {
    pub gates: Vec<RaceGate>,
    /// True once the player has crossed the start line and the timer is running.
    pub running: bool,
    /// Ticks elapsed since the start line (converted to ms externally).
    pub ticks: u32,
    /// Next checkpoint index the player must pass (gates in order).
    pub next_checkpoint: usize,
    /// Split times (in ticks) for each passed checkpoint.
    pub splits: Vec<u32>,
    /// Final race time (in ticks), set once finished.
    pub finished_ticks: Option<u32>,
    /// Whether a finish has been reached (race complete).
    pub finished: bool,
}

impl Race {
    fn new(gates: Vec<RaceGate>) -> Race {
        Race {
            gates,
            running: false,
            ticks: 0,
            next_checkpoint: 0,
            splits: Vec::new(),
            finished_ticks: None,
            finished: false,
        }
    }

    /// Reset the race (restart from the beginning).
    fn reset(&mut self) {
        self.running = false;
        self.ticks = 0;
        self.next_checkpoint = 0;
        self.splits.clear();
        self.finished_ticks = None;
        self.finished = false;
    }
}

/// A playable session bound to one loaded map.
pub struct Session {
    pmove: Pmove,
    ps: PlayerState,
    angles: Angles,
    mouse: MouseConfig,
    held_cmd: Cmd,
    spawn_origin: [f32; 3],
    spawn_yaw: f32,
    spawn_pitch: f32,
    race: Race,
    /// Bounds the position-save zone (the start gate AABB). Outside this the
    /// player cannot set a new spawn. Updated as the race passes checkpoints.
    save_zone: Option<[[f32; 3]; 2]>,
    /// The "before the start line" side: the axis (0/1/2) of the start gate's
    /// thin dimension, and the sign of the spawn's offset from the gate center
    /// along that axis. Position-save must land on the same side (behind the
    /// start line), not past it.
    start_axis: usize,
    start_sign: f32,
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
        let (spawn_origin, spawn_yaw) = if let Some(sp) = bsp.spawns.get(spawn_index) {
            (sp.origin, sp.yaw)
        } else {
            ([0.0, 0.0, 64.0], 0.0f32)
        };
        ps.origin = spawn_origin;
        ps.viewangles = [0.0, spawn_yaw, 0.0];

        // Drop onto the ground.
        let mut pmove = Pmove::new(world, frametime);
        pmove.drop_to_ground(&mut ps);

        let yaw = ps.viewangles[1];
        let race = Race::new(bsp.race_gates.clone());
        // Initialize the view angles from the spawn yaw (radians) via the
        // fixed-point conversion, so spawn orientation matches `set_view_rad`
        // (forward = +X at yaw 0). Putting radians directly into `Angles.yaw`
        // (a 16-bit fixed-point angle) flips the player 180°, facing them into
        // the wall behind spawn instead of toward the start line.
        let mut angles = Angles::default();
        angles.set_view_rad(yaw, 0.0);
        // The save zone is a generous box around the start gate trigger, but we
        // restrict saving to the "before" side of the start line (the side the
        // spawn is on), so you can't save past it.
        const SAVE_MARGIN: f32 = 256.0;
        let (save_zone, start_axis, start_sign) = match bsp
            .race_gates
            .iter()
            .find(|g| g.kind == RaceGateKind::Start)
        {
            Some(g) => {
                let zone = [
                    [g.mins[0] - SAVE_MARGIN, g.mins[1] - SAVE_MARGIN, g.mins[2] - SAVE_MARGIN],
                    [g.maxs[0] + SAVE_MARGIN, g.maxs[1] + SAVE_MARGIN, g.maxs[2] + SAVE_MARGIN],
                ];
                // Thin axis = smallest extent of the start gate.
                let ext = [
                    g.maxs[0] - g.mins[0],
                    g.maxs[1] - g.mins[1],
                    g.maxs[2] - g.mins[2],
                ];
                let axis = if ext[0] <= ext[1] && ext[0] <= ext[2] {
                    0
                } else if ext[1] <= ext[2] {
                    1
                } else {
                    2
                };
                let center = (g.mins[axis] + g.maxs[axis]) * 0.5;
                let sign = if spawn_origin[axis] < center { -1.0 } else { 1.0 };
                (Some(zone), axis, sign)
            }
            None => (None, 0, 0.0),
        };

        Ok(Session {
            pmove,
            ps,
            angles,
            mouse: MouseConfig::default(),
            held_cmd: Cmd::default(),
            spawn_origin,
            spawn_yaw,
            spawn_pitch: 0.0,
            race,
            save_zone,
            start_axis,
            start_sign,
        })
    }

    /// Reset the player back to the spawn point (race restart).
    pub fn reset(&mut self) {
        self.ps.origin = self.spawn_origin;
        self.ps.velocity = [0.0, 0.0, 0.0];
        self.ps.viewangles = [self.spawn_pitch, self.spawn_yaw, 0.0];
        self.ps.doshtime = 0;
        self.ps.wjtime = 0;
        self.ps.on_ground = false;
        self.ps.special_held = false;
        self.ps.jump_held = false;
        self.angles.set_view_rad(self.spawn_yaw, self.spawn_pitch);
        self.held_cmd = Cmd::default();
        self.pmove.drop_to_ground(&mut self.ps);
        self.race.reset();
    }

    pub fn set_sensitivity(&mut self, s: f32) {
        self.mouse.sensitivity = s;
    }

    /// Directly set the player position and view (debug/noclip / test hook).
    pub fn teleport(&mut self, x: f32, y: f32, z: f32, yaw: f32, pitch: f32) {
        self.ps.origin = [x, y, z];
        self.ps.velocity = [0.0, 0.0, 0.0];
        self.ps.viewangles = [pitch, yaw, 0.0];
        self.ps.on_ground = false;
        self.angles.set_view_rad(yaw, pitch);
        self.held_cmd = Cmd::default();
        self.pmove.drop_to_ground(&mut self.ps);
    }

    /// Set position without gravity settle (test hook for exact repro).
    pub fn teleport_raw(&mut self, x: f32, y: f32, z: f32, yaw: f32, pitch: f32) {
        self.ps.origin = [x, y, z];
        self.ps.velocity = [0.0, 0.0, 0.0];
        self.ps.viewangles = [pitch, yaw, 0.0];
        self.ps.on_ground = false;
        self.angles.set_view_rad(yaw, pitch);
        self.held_cmd = Cmd::default();
    }

    /// Save the current position as the new spawn point. Returns true on
    /// success. Only allowed within the current save zone (the start gate), on
    /// the "before the start line" side (same side as the original spawn), so
    /// you can't start past the line.
    pub fn position_save(&mut self) -> bool {
        let Some([mins, maxs]) = self.save_zone else {
            return false;
        };
        let p = self.ps.origin;
        let inside = p[0] >= mins[0]
            && p[0] <= maxs[0]
            && p[1] >= mins[1]
            && p[1] <= maxs[1]
            && p[2] >= mins[2]
            && p[2] <= maxs[2];
        if !inside {
            return false;
        }
        // Must be on the "before" side of the start line (same side as the
        // original spawn), i.e. the same sign along the gate's thin axis
        // relative to the gate center.
        let center = (mins[self.start_axis] + maxs[self.start_axis]) * 0.5;
        let player_side = if p[self.start_axis] < center { -1.0 } else { 1.0 };
        if player_side != self.start_sign {
            return false; // past the start line
        }
        self.spawn_origin = p;
        self.spawn_yaw = self.ps.viewangles[1];
        self.spawn_pitch = self.ps.viewangles[0];
        true
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

        // Race timer: advance and detect gate passes.
        if self.race.running && !self.race.finished {
            self.race.ticks = self.race.ticks.saturating_add(1);
        }
        self.check_gates();
    }

    /// Detect gate passes (start/checkpoints/finish) via player AABB overlap.
    fn check_gates(&mut self) {
        if self.race.finished {
            return;
        }
        // Player box (full extent) for overlap testing.
        let mins = crate::trace::PLAYER_MINS;
        let maxs = crate::trace::PLAYER_MAXS;
        let pmin = [
            self.ps.origin[0] + mins[0],
            self.ps.origin[1] + mins[1],
            self.ps.origin[2] + mins[2],
        ];
        let pmax = [
            self.ps.origin[0] + maxs[0],
            self.ps.origin[1] + maxs[1],
            self.ps.origin[2] + maxs[2],
        ];

        let gates = self.race.gates.clone();
        for (i, gate) in gates.iter().enumerate() {
            let overlaps = pmin[0] <= gate.maxs[0] && pmax[0] >= gate.mins[0]
                && pmin[1] <= gate.maxs[1] && pmax[1] >= gate.mins[1]
                && pmin[2] <= gate.maxs[2] && pmax[2] >= gate.mins[2];
            if !overlaps {
                continue;
            }
            match gate.kind {
                RaceGateKind::Start => {
                    if !self.race.running {
                        self.race.running = true;
                        self.race.ticks = 0;
                        self.race.next_checkpoint = 0;
                        self.race.splits.clear();
                    }
                }
                RaceGateKind::Checkpoint => {
                    // Must be the next checkpoint in sequence.
                    if self.race.running && i <= self.race.gates.len() {
                        // Determine how many checkpoints have been passed.
                        // We use `self.race.next_checkpoint` as the count.
                        let cp_positions: Vec<usize> = self
                            .race
                            .gates
                            .iter()
                            .enumerate()
                            .filter(|(_, g)| g.kind == RaceGateKind::Checkpoint)
                            .map(|(idx, _)| idx)
                            .collect();
                        if let Some(pos) = cp_positions.iter().position(|&idx| idx == i) {
                            if pos == self.race.next_checkpoint {
                                self.race.splits.push(self.race.ticks);
                                self.race.next_checkpoint += 1;
                            }
                        }
                    }
                }
                RaceGateKind::Finish => {
                    if self.race.running {
                        self.race.finished_ticks = Some(self.race.ticks);
                        self.race.finished = true;
                        self.race.running = false;
                    }
                }
            }
        }
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

    /// Camera eye position (render space). The eye sits `viewheight` above the
    /// player origin. Warfork: stand `playerbox_stand_viewheight = 30`, crouch
    /// `playerbox_crouch_viewheight = 12`. (Our crouch eye used to be 18, which
    /// is *above* the crouched box top (origin+16), so the camera poked into low
    /// objects and made the player feel too tall to fit under them.)
    pub fn eye(&self) -> [f32; 3] {
        let off = if self.ps.crouched { 12.0 } else { 30.0 };
        [self.ps.origin[0], self.ps.origin[1], self.ps.origin[2] + off]
    }

    pub fn yaw(&self) -> f32 { self.ps.viewangles[1] }
    pub fn pitch(&self) -> f32 { self.ps.viewangles[0] }
    pub fn speed(&self) -> f32 { self.ps.speed }
    pub fn on_ground(&self) -> bool { self.ps.on_ground }
    pub fn origin(&self) -> [f32; 3] { self.ps.origin }
    pub fn velocity(&self) -> [f32; 3] { self.ps.velocity }

    /// Movement HUD diagnostics: the data the client needs to render strafe /
    /// bunny turn indicators and an acceleration bar.
    ///
    /// Returns 10 floats:
    ///   [0..2] = horizontal velocity (vx, vy, 0)
    ///   [3..5] = horizontal wish direction (normalized), (0,0,0) if no input
    ///   [6]    = horizontal speed
    ///   [7]    = signed accel: dot(velocity, wishdir)
    ///   [8]    = forward input (-1..1)
    ///   [9]    = strafe input (-1..1, +right)
    pub fn movement_hint(&self) -> Vec<f32> {
        let yaw = self.ps.viewangles[1];
        let (sy, cy) = yaw.sin_cos();
        let forward = [cy, sy];
        let right = [sy, -cy];

        let cmd = &self.held_cmd;
        let fwd = cmd.forward as f32 / 127.0;
        let strafe = cmd.right as f32 / 127.0;

        let mut wx = forward[0] * fwd + right[0] * strafe;
        let mut wy = forward[1] * fwd + right[1] * strafe;
        let wlen = (wx * wx + wy * wy).sqrt();
        if wlen > 1e-6 {
            wx /= wlen;
            wy /= wlen;
        } else {
            wx = 0.0;
            wy = 0.0;
        }

        let vx = self.ps.velocity[0];
        let vy = self.ps.velocity[1];
        let speed = (vx * vx + vy * vy).sqrt();
        // Signed acceleration along the wish direction (positive = gaining).
        let accel = vx * wx + vy * wy;

        vec![vx, vy, 0.0, wx, wy, 0.0, speed, accel, fwd, strafe]
    }

    // Race state getters.
    pub fn race_running(&self) -> bool { self.race.running }
    pub fn race_finished(&self) -> bool { self.race.finished }
    /// Elapsed race ticks (convert to ms with * 1000 / TICK_RATE).
    pub fn race_ticks(&self) -> u32 { self.race.ticks }
    /// Final race time in ticks, if finished.
    pub fn race_finished_ticks(&self) -> Option<u32> { self.race.finished_ticks }
    /// Number of checkpoints passed (split count).
    pub fn race_split_count(&self) -> usize { self.race.splits.len() }
    /// Split times (ticks) for each passed checkpoint.
    pub fn race_splits(&self) -> Vec<u32> { self.race.splits.clone() }
    /// Total number of checkpoints on this map.
    pub fn race_total_checkpoints(&self) -> usize {
        self.race
            .gates
            .iter()
            .filter(|g| g.kind == RaceGateKind::Checkpoint)
            .count()
    }
}
