//! Warfork movement port — `PM_Accelerate`, `PM_AirAccelerate`, `PM_Aircontrol`,
//! ground/air move, dash, wall-jump, jump (double-jump), overbounce.
//!
//! Faithful port of `source/common/facilities/gs_pmove.cpp` with the exact
//! constants from `lib.rs`. The `frametime` here is the per-tick delta
//! (1/250 s at our tick rate).

#![allow(dead_code)]

use crate::input::Cmd;
use crate::trace::World;
use crate::{
    CROUCH_SPEED, DASHJUMP_TIMEDELAY, GRAVITY, PM_ACCELERATE,
    PM_AIRCONTROL, PM_DASH_UPSPEED, PM_FRICTION, PM_OVERBOUNCE,
    PM_STRANGE_BUNNY_ACCEL, PM_WISHSPEED, PM_WJ_BOUNCE, PM_WJ_UPSPEED, WALK_SPEED,
};

pub const PM_DASHTIME_MASK: u32 = 0xffff;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PmType {
    Normal,
    Spectator,
}

pub struct PlayerState {
    pub origin: [f32; 3],
    pub velocity: [f32; 3],
    pub viewangles: [f32; 3],
    pub doshtime: u32,
    pub wjtime: u32,
    pub crouchtime: u32,
    pub special_held: bool,
    pub jump_held: bool,
    pub on_ground: bool,
    // Velocity magnitude last tick (for overbounce / speed display).
    pub speed: f32,
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            origin: [0.0, 0.0, 64.0],
            velocity: [0.0, 0.0, 0.0],
            viewangles: [0.0, 0.0, 0.0],
            doshtime: 0,
            wjtime: 0,
            crouchtime: 0,
            special_held: false,
            jump_held: false,
            on_ground: false,
            speed: 0.0,
        }
    }
}

pub struct Pmove {
    pub world: World,
    pub frametime: f32,
    pub max_speed: f32,
    pub max_player_speed: f32,
}

impl Pmove {
    pub fn new(world: World, frametime: f32) -> Self {
        // `max_speed` is the player run speed used by PM_CmdScale (the entity
        // "speed" field, 320 ups — Quake/Warsow standard). `max_player_speed`
        // is the air-bunny reference speed (Warfork's `maxPlayerSpeed`).
        Pmove {
            world,
            frametime,
            max_speed: 320.0,
            max_player_speed: 600.0,
        }
    }

    /// Repeatedly step the player downward (no input) until grounded, so a
    /// spawn point that hovers above the floor drops to it.
    pub fn drop_to_ground(&mut self, ps: &mut PlayerState) {
        for _ in 0..256 {
            if ps.on_ground {
                break;
            }
            let cmd = Cmd::default();
            // Apply gravity.
            ps.velocity[2] -= GRAVITY * self.frametime;
            self.slide_move(ps);
            self.update_ground(ps);
            let _ = cmd;
        }
    }

    /// Advance one tick.
    pub fn step(&mut self, ps: &mut PlayerState, cmd: &Cmd) {
        let special = cmd.buttons & crate::input::BUTTON_SPECIAL != 0;

        // Timers decay (in milliseconds).
        if ps.doshtime > 0 {
            ps.doshtime = ps.doshtime.saturating_sub((self.frametime * 1000.0) as u32);
        }
        if ps.wjtime > 0 {
            ps.wjtime = ps.wjtime.saturating_sub((self.frametime * 1000.0) as u32);
        }

        // Compute forward/right/up from view yaw (pitch ignored for horizontal).
        let yaw = ps.viewangles[1];
        let (sy, cy) = yaw.sin_cos();
        let forward = [cy, sy, 0.0];
        let right = [sy, -cy, 0.0];
        let up = [0.0, 0.0, 1.0];

        let fwd_push = cmd.forward as f32;
        let side_push = cmd.right as f32;
        let up_push = cmd.up as f32;

        // Jump.
        if cmd.buttons & crate::input::BUTTON_JUMP != 0 && !ps.jump_held && ps.on_ground {
            self.jump(ps, forward);
        }
        ps.jump_held = cmd.buttons & crate::input::BUTTON_JUMP != 0;

        // Wall-jump.
        if special {
            self.walljump(ps, cmd, right, forward, side_push, fwd_push);
        }

        // Dash.
        if special {
            self.dash(ps, cmd, right, forward, side_push, fwd_push);
        }

        // Ground friction / movement.
        if ps.on_ground {
            self.ground_move(ps, forward, right, fwd_push, side_push);
        } else {
            self.air_move(ps, forward, right, fwd_push, side_push);
        }

        // Gravity.
        if !ps.on_ground {
            ps.velocity[2] -= GRAVITY * self.frametime;
        }

        // Integrate position with collision.
        self.slide_move(ps);

        ps.speed = (ps.velocity[0] * ps.velocity[0]
            + ps.velocity[1] * ps.velocity[1]
            + ps.velocity[2] * ps.velocity[2])
            .sqrt();

        // Recompute ground contact.
        self.update_ground(ps);
        let _ = up_push;
        let _ = up;
    }

    fn jump(&mut self, ps: &mut PlayerState, _forward: [f32; 3]) {
        // Warfork jump velocity (jumpPlayerSpeed = 270 * compensate).
        let jump_speed = 270.0 * crate::GRAVITY_COMPENSATE;
        if ps.velocity[2] > 100.0 {
            // double jump
            ps.velocity[2] += jump_speed;
        } else if ps.velocity[2] > 0.0 {
            ps.velocity[2] += jump_speed;
        } else {
            ps.velocity[2] = jump_speed;
        }
        ps.on_ground = false;
        ps.doshtime = 0;
        ps.wjtime = 0;
    }

    fn dash(
        &mut self,
        ps: &mut PlayerState,
        cmd: &Cmd,
        right: [f32; 3],
        forward: [f32; 3],
        side_push: f32,
        fwd_push: f32,
    ) {
        if ps.doshtime > 0 || !ps.on_ground {
            return;
        }
        if !(cmd.buttons & crate::input::BUTTON_SPECIAL != 0) {
            return;
        }
        if ps.special_held {
            return;
        }
        ps.special_held = true;

        // dash direction from movement keys; default forward.
        let mut dir = [
            forward[0] * fwd_push + right[0] * side_push,
            forward[1] * fwd_push + right[1] * side_push,
            0.0,
        ];
        let len2 = dir[0] * dir[0] + dir[1] * dir[1];
        if len2 < 0.0001 {
            dir = [forward[0], forward[1], 0.0];
        }
        let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
        if len > 0.0 {
            dir[0] /= len;
            dir[1] /= len;
        }

        // preserve horizontal speed (dash doesn't reduce it below dash speed).
        let hspeed = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1])
            .sqrt();
        let target = hspeed.max(450.0); // dashPlayerSpeed ~ 450 ups in Warfork
        ps.velocity[0] = dir[0] * target;
        ps.velocity[1] = dir[1] * target;

        // upward kick.
        let up_speed = if ps.velocity[2] <= 0.0 {
            PM_DASH_UPSPEED
        } else {
            PM_DASH_UPSPEED + ps.velocity[2]
        };
        ps.velocity[2] = up_speed;

        ps.doshtime = DASHJUMP_TIMEDELAY;
        ps.on_ground = false;
    }

    fn walljump(
        &mut self,
        ps: &mut PlayerState,
        cmd: &Cmd,
        right: [f32; 3],
        _forward: [f32; 3],
        side_push: f32,
        _fwd_push: f32,
    ) {
        if !(cmd.buttons & crate::input::BUTTON_SPECIAL != 0) {
            return;
        }
        if ps.on_ground || ps.wjtime > 0 {
            return;
        }
        if ps.special_held {
            return;
        }

        // Determine which wall we're pushing against by tracing sideways.
        let dir = if side_push != 0.0 {
            [right[0] * side_push.signum(), right[1] * side_push.signum(), 0.0]
        } else {
            return;
        };

        let start = ps.origin;
        let end = [start[0] + dir[0] * 32.0, start[1] + dir[1] * 32.0, start[2]];
        let tr = self.world.trace(
            start,
            crate::trace::PLAYER_MINS,
            crate::trace::PLAYER_MAXS,
            end,
        );
        if tr.fraction >= 1.0 {
            return; // no wall
        }

        let n = tr.normal;
        ps.special_held = true;

        // Wall-jump: bounce off the wall (horizontal refliction + upward).
        let dot = ps.velocity[0] * n[0] + ps.velocity[1] * n[1];
        let mut bounced = [
            ps.velocity[0] - 2.0 * dot * n[0],
            ps.velocity[1] - 2.0 * dot * n[1],
            ps.velocity[2],
        ];
        // warfork: horizontal speed is scaled by bounce factor + min speed.
        let h = (bounced[0] * bounced[0] + bounced[1] * bounced[1]).sqrt();
        let min_speed = (WALK_SPEED + self.max_speed) * 0.5;
        if h < min_speed {
            let hh = if h > 0.0 { h } else { 1.0 };
            bounced[0] *= min_speed / hh;
            bounced[1] *= min_speed / hh;
        }
        if bounced[2] < PM_WJ_UPSPEED {
            bounced[2] = PM_WJ_UPSPEED;
        }
        // Reduce horizontal velocity on bounce.
        bounced[0] *= PM_WJ_BOUNCE;
        bounced[1] *= PM_WJ_BOUNCE;

        ps.velocity = bounced;
        ps.wjtime = crate::WALLJUMP_TIMEDELAY;
    }

    fn ground_move(
        &mut self,
        ps: &mut PlayerState,
        forward: [f32; 3],
        right: [f32; 3],
        fwd: f32,
        side: f32,
    ) {
        // Apply ground friction (Quake PM_Friction).
        let speed = (ps.velocity[0] * ps.velocity[0]
            + ps.velocity[1] * ps.velocity[1]
            + ps.velocity[2] * ps.velocity[2])
            .sqrt();
        if speed > 0.0 {
            let control = if speed < 1.0 { 1.0 } else { speed };
            let drop = control * PM_FRICTION * self.frametime;
            let newspeed = (speed - drop).max(0.0) / speed.max(0.0001);
            ps.velocity[0] *= newspeed;
            ps.velocity[1] *= newspeed;
            ps.velocity[2] *= newspeed;
        }

        // Project forward/right onto the flat plane and build the wish dir.
        let mut wishvel = [
            forward[0] * fwd + right[0] * side,
            forward[1] * fwd + right[1] * side,
            0.0,
        ];
        let len = (wishvel[0] * wishvel[0] + wishvel[1] * wishvel[1]).sqrt();
        let mut wishspeed = 0.0;
        if len > 0.0 {
            wishvel[0] /= len;
            wishvel[1] /= len;
            // Quake PM_CmdScale + VectorNormalize cancel the `total` factor:
            // wishspeed = max_speed * max_mag / 127 (max_mag is the largest
            // key magnitude, full press = 127).
            let max_mag = fwd.abs().max(side.abs());
            wishspeed = self.max_speed * max_mag / 127.0;
        }
        wishspeed = wishspeed.min(self.max_speed);

        self.accelerate(ps, wishvel, wishspeed, PM_ACCELERATE);
    }

    fn air_move(
        &mut self,
        ps: &mut PlayerState,
        forward: [f32; 3],
        right: [f32; 3],
        fwd: f32,
        side: f32,
    ) {
        let mut wishvel = [
            forward[0] * fwd + right[0] * side,
            forward[1] * fwd + right[1] * side,
            0.0,
        ];
        let len = (wishvel[0] * wishvel[0] + wishvel[1] * wishvel[1]).sqrt();
        let mut wishspeed = 0.0;
        if len > 0.0 {
            wishvel[0] /= len;
            wishvel[1] /= len;
            let max_mag = fwd.abs().max(side.abs());
            wishspeed = self.max_speed * max_mag / 127.0;
        }
        wishspeed = wishspeed.min(self.max_speed);

        // Strafe-bunny short-hop: if strafing (no forward) cap wishspeed to
        // pm_wishspeed and use the bunny accel (mirrors Warfork air control).
        if side != 0.0 && fwd == 0.0 {
            if wishspeed > PM_WISHSPEED {
                wishspeed = PM_WISHSPEED;
            }
            self.accelerate(ps, wishvel, wishspeed, PM_STRANGE_BUNNY_ACCEL);
        } else {
            // Forward + air movement: use the forward-bunny model which lets
            // horizontal speed climb toward bunnytopspeed (Warfork fwdbunny).
            self.air_accelerate(ps, wishvel, wishspeed);
        }

        if PM_AIRCONTROL != 0.0 {
            self.aircontrol(ps, wishvel, wishspeed);
        }
    }

    /// `PM_AirAccelerate` (Warfork fwdbunny): accelerates horizontal velocity
    /// toward the wish direction with a soft cap at `bunnytopspeed` = 925 ups.
    fn air_accelerate(&mut self, ps: &mut PlayerState, wishdir: [f32; 3], wishspeed: f32) {
        const AIRFORWARDACCEL: f32 = 1.00001;
        const BUNNYACCEL: f32 = 0.1593;
        const BUNNYTOPSPEED: f32 = 925.0;
        const TURNACCEL: f32 = 4.0;
        const BACKTOSIDERATIO: f32 = 0.8;

        if wishspeed == 0.0 {
            return;
        }

        let curvel = [ps.velocity[0], ps.velocity[1], 0.0];
        let curspeed = (curvel[0] * curvel[0] + curvel[1] * curvel[1]).sqrt();

        let mut wspeed = wishspeed;
        if wspeed > curspeed * 1.01 {
            // moving below max_speed: accelerate quickly up to it.
            let accelspeed = curspeed + AIRFORWARDACCEL * self.max_player_speed * self.frametime;
            if accelspeed < wspeed {
                wspeed = accelspeed;
            }
        } else {
            let mut f = (BUNNYTOPSPEED - curspeed) / (BUNNYTOPSPEED - self.max_player_speed);
            if f < 0.0 {
                f = 0.0;
            }
            wspeed = curspeed.max(self.max_player_speed)
                + BUNNYACCEL * f * self.max_player_speed * self.frametime;
        }

        let wishvel = [wishdir[0] * wspeed, wishdir[1] * wspeed, 0.0];
        let mut acceldir = [wishvel[0] - curvel[0], wishvel[1] - curvel[1], 0.0];
        let addspeed = (acceldir[0] * acceldir[0] + acceldir[1] * acceldir[1]).sqrt();
        if addspeed > 0.0 {
            acceldir[0] /= addspeed;
            acceldir[1] /= addspeed;
        } else {
            return;
        }

        let mut accelspeed = TURNACCEL * self.max_player_speed * self.frametime;
        if accelspeed > addspeed {
            accelspeed = addspeed;
        }

        // backtosideratio: soften acceleration when turning against momentum.
        if BACKTOSIDERATIO < 1.0 && curspeed > 0.0 {
            let curdir = [curvel[0] / curspeed, curvel[1] / curspeed];
            let dot = acceldir[0] * curdir[0] + acceldir[1] * curdir[1];
            if dot < 0.0 {
                acceldir[0] -= (1.0 - BACKTOSIDERATIO) * dot * curdir[0];
                acceldir[1] -= (1.0 - BACKTOSIDERATIO) * dot * curdir[1];
            }
        }

        ps.velocity[0] += accelspeed * acceldir[0];
        ps.velocity[1] += accelspeed * acceldir[1];
    }

    fn accelerate(&mut self, ps: &mut PlayerState, wishdir: [f32; 3], wishspeed: f32, accel: f32) {
        let current = ps.velocity[0] * wishdir[0]
            + ps.velocity[1] * wishdir[1]
            + ps.velocity[2] * wishdir[2];
        let addspeed = wishspeed - current;
        if addspeed <= 0.0 {
            return;
        }
        let mut accelspeed = accel * self.frametime * wishspeed;
        if accelspeed > addspeed {
            accelspeed = addspeed;
        }
        ps.velocity[0] += accelspeed * wishdir[0];
        ps.velocity[1] += accelspeed * wishdir[1];
        ps.velocity[2] += accelspeed * wishdir[2];
    }

    fn aircontrol(&mut self, ps: &mut PlayerState, wishdir: [f32; 3], wishspeed: f32) {
        // No air control if not moving forward.
        let smove = 0.0; // side movement already handled; replicate Warfork +strafe
        if smove != 0.0 || wishspeed == 0.0 {
            return;
        }
        let zspeed = ps.velocity[2];
        ps.velocity[2] = 0.0;
        let speed = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1])
            .sqrt();
        if speed == 0.0 {
            ps.velocity[2] = zspeed;
            return;
        }
        let dot = (ps.velocity[0] / speed) * wishdir[0] + (ps.velocity[1] / speed) * wishdir[1];
        let k = 32.0 * PM_AIRCONTROL * dot * dot * self.frametime;
        if dot > 0.0 {
            ps.velocity[0] = ps.velocity[0] * speed + wishdir[0] * k;
            ps.velocity[1] = ps.velocity[1] * speed + wishdir[1] * k;
            let ns = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1])
                .sqrt();
            if ns > 0.0 {
                ps.velocity[0] = ps.velocity[0] / ns * speed;
                ps.velocity[1] = ps.velocity[1] / ns * speed;
            }
        }
        ps.velocity[2] = zspeed;
    }

    /// Slide the player along the world, resolving collision (simplified
    /// `PM_StepSlideMove` — one trace + one reflection per tick; enough for
    /// correct strafe-jumping on flat/quasi-flat geometry).
    fn slide_move(&mut self, ps: &mut PlayerState) {
        let start = ps.origin;
        let mut end = [
            start[0] + ps.velocity[0] * self.frametime,
            start[1] + ps.velocity[1] * self.frametime,
            start[2] + ps.velocity[2] * self.frametime,
        ];
        let mins = crate::trace::PLAYER_MINS;
        let maxs = crate::trace::PLAYER_MAXS;

        // First, allow stepping (like PM_StepSlideMove): try to step up stairs.
        let mut tr = self.world.trace(start, mins, maxs, end);
        if tr.start_solid || tr.fraction < 1.0 {
            // try step up (18 units, Quake default step)
            let up_delta = 18.0;
            let mut step_end = [start[0], start[1], start[2] + up_delta];
            let v = [
                ps.velocity[0] * self.frametime,
                ps.velocity[1] * self.frametime,
                0.0,
            ];
            step_end[0] += v[0];
            step_end[1] += v[1];
            let step_tr = self.world.trace(start, mins, maxs, step_end);
            if step_tr.fraction < 1.0 && !step_tr.start_solid {
                let at = step_end;
                let down_end = [at[0], at[1], at[2] - up_delta];
                let down_tr = self.world.trace(at, mins, maxs, down_end);
                if down_tr.fraction < 1.0 {
                    end = [
                        at[0],
                        at[1],
                        at[2] - (1.0 - down_tr.fraction) * up_delta,
                    ];
                    tr = down_tr;
                    ps.origin = end;
                    // Clear downward velocity from the step.
                    if ps.velocity[2] < 0.0 {
                        ps.velocity[2] = 0.0;
                    }
                    // horizontal velocity preserved by slide below
                }
            }
        }

        if tr.fraction >= 1.0 {
            ps.origin = end;
            return;
        }

        if tr.fraction > 0.0 {
            end = [
                start[0] * (1.0 - tr.fraction) + end[0] * tr.fraction + tr.normal[0] * 0.01,
                start[1] * (1.0 - tr.fraction) + end[1] * tr.fraction + tr.normal[1] * 0.01,
                start[2] * (1.0 - tr.fraction) + end[2] * tr.fraction + tr.normal[2] * 0.01,
            ];
        }
        ps.origin = end;

        // Clip velocity to the plane (overbounce factor 1.01).
        let n = tr.normal;
        let dot = ps.velocity[0] * n[0] + ps.velocity[1] * n[1] + ps.velocity[2] * n[2];
        if dot < 0.0 {
            ps.velocity[0] -= n[0] * dot * PM_OVERBOUNCE;
            ps.velocity[1] -= n[1] * dot * PM_OVERBOUNCE;
            ps.velocity[2] -= n[2] * dot * PM_OVERBOUNCE;
        }
    }

    fn update_ground(&mut self, ps: &mut PlayerState) {
        // A downward trace just below the origin determines ground plane.
        let start = ps.origin;
        let end = [start[0], start[1], start[2] - 1.0];
        let tr = self.world.trace(
            start,
            crate::trace::PLAYER_MINS,
            crate::trace::PLAYER_MAXS,
            end,
        );
        ps.on_ground = tr.fraction < 1.0;
    }
}

/// Convenience for tests: expose the crouch max speed.
pub const CROUCH_SPEEDV: f32 = CROUCH_SPEED;
