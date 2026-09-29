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
    CROUCH_SPEED, DASHJUMP_TIMEDELAY, GRAVITY, PM_ACCELERATE, PM_AIRACCELERATE,
    PM_AIRCONTROL, PM_DASH_UPSPEED, PM_FRICTION,
    PM_STRANGE_BUNNY_ACCEL, PM_WISHSPEED, PM_WJ_UPSPEED, WALK_SPEED,
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
    /// Surface flags of the ground currently stood on (SURF_SLICK etc.).
    pub ground_flags: i32,
    /// Normal of the ground plane currently stood on (for slope sliding).
    pub ground_normal: [f32; 3],
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
            ground_flags: 0,
            ground_normal: [0.0, 0.0, 1.0],
            speed: 0.0,
        }
    }
}

/// Surface flags (mirrors QFusion qfiles.h).
pub const SURF_SLICK: i32 = 0x2;

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

    /// Settle the player onto the ground. Handles spawn points floating above
    /// the floor or embedded in geometry by simulating a short drop; then
    /// zeroes velocity so the player starts clean.
    pub fn drop_to_ground(&mut self, ps: &mut PlayerState) {
        const PLAYER_MINS: [f32; 3] = crate::trace::PLAYER_MINS;
        const PLAYER_MAXS: [f32; 3] = crate::trace::PLAYER_MAXS;

        // If embedded (start_solid), nudge up until free.
        for _ in 0..256 {
            let tr = self.world.trace(ps.origin, PLAYER_MINS, PLAYER_MAXS, ps.origin);
            if !tr.start_solid {
                break;
            }
            ps.origin[2] += 2.0;
        }

        // Simulate falling until the feet come to rest on the surface.
        for _ in 0..512 {
            ps.velocity[2] -= GRAVITY * self.frametime;
            self.slide_move(ps);
            if self.grounded(ps) && ps.velocity[2].abs() < 20.0 {
                break;
            }
        }

        ps.velocity = [0.0, 0.0, 0.0];
        ps.speed = 0.0;
        // Final: resolve any remaining embedding by nudging up a bit.
        for _ in 0..64 {
            let tr = self.world.trace(ps.origin, PLAYER_MINS, PLAYER_MAXS, ps.origin);
            if !tr.start_solid {
                break;
            }
            ps.origin[2] += 1.0;
        }
        ps.on_ground = true;
    }

    /// Whether the player's feet are resting on a floor-like surface (a hit
    /// whose normal points mostly up). A wall or steep slope does NOT count as
    /// ground — this prevents the player from "sticking" and floating when
    /// pressed against walls. Also records the ground surface's flags (slick).
    fn grounded(&mut self, ps: &mut PlayerState) -> bool {
        // Never grounded while rising — a player who just jumped is airborne
        // even if their box still overlaps the floor's down-trace margin.
        if ps.velocity[2] > 20.0 {
            ps.ground_flags = 0;
            ps.ground_normal = [0.0, 0.0, 1.0];
            return false;
        }

        let down = 2.0;
        let start = ps.origin;
        let end = [ps.origin[0], ps.origin[1], ps.origin[2] - down];
        let tr = self.world.trace(
            start,
            crate::trace::PLAYER_MINS,
            crate::trace::PLAYER_MAXS,
            end,
        );
        // Grounded only if we actually hit something walkable (flat enough).
        if tr.fraction >= 1.0 {
            ps.ground_flags = 0;
            ps.ground_normal = [0.0, 0.0, 1.0];
            return false;
        }
        if tr.normal[2] > 0.7 {
            ps.ground_flags = tr.surface_flags;
            ps.ground_normal = tr.normal;
            return true;
        }
        ps.ground_flags = 0;
        ps.ground_normal = [0.0, 0.0, 1.0];
        false
    }

    /// Advance one tick.
    pub fn step(&mut self, ps: &mut PlayerState, cmd: &Cmd) {
        let special = cmd.buttons & crate::input::BUTTON_SPECIAL != 0;
        let jump = cmd.buttons & crate::input::BUTTON_JUMP != 0;

        // Timers decay (in milliseconds).
        if ps.doshtime > 0 {
            ps.doshtime = ps.doshtime.saturating_sub((self.frametime * 1000.0) as u32);
        }
        if ps.wjtime > 0 {
            ps.wjtime = ps.wjtime.saturating_sub((self.frametime * 1000.0) as u32);
        }

        // Release the special-held latch when the button is up.
        if !special {
            ps.special_held = false;
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

        // Jump — continuous (autohop): while held and grounded, keep hopping.
        if jump && ps.on_ground {
            self.jump(ps, forward);
        }
        ps.jump_held = jump;

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

        // Gravity. Always applied; the ground-plane clip (below) holds the
        // player on the floor and redirects the pull into downhill motion on
        // slopes (which is what slick ramps need for acceleration).
        ps.velocity[2] -= GRAVITY * self.frametime;

        // When grounded, clip velocity against the ground plane so the player
        // stays on the surface and the gravity component becomes slide motion.
        if ps.on_ground {
            let n = ps.ground_normal;
            let dot = ps.velocity[0] * n[0] + ps.velocity[1] * n[1] + ps.velocity[2] * n[2];
            if dot < 0.0 {
                ps.velocity[0] -= n[0] * dot;
                ps.velocity[1] -= n[1] * dot;
                ps.velocity[2] -= n[2] * dot;
            }
        }

        // Integrate position with collision.
        self.slide_move(ps);

        // Horizontal speed only (ups = forward/strafe speed, not vertical).
        ps.speed = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();

        // Recompute ground contact.
        self.update_ground(ps);
        let _ = up_push;
        let _ = up;
    }

    fn jump(&mut self, ps: &mut PlayerState, _forward: [f32; 3]) {
        // Warfork jump (DEFAULT_JUMPSPEED = 280 * gravity compensation).
        let jump_speed = crate::JUMP_SPEED * crate::GRAVITY_COMPENSATE;
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
        forward: [f32; 3],
        side_push: f32,
        fwd_push: f32,
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

        // Direction to check for a wall: the player's horizontal movement
        // direction, falling back to their input direction.
        let hvel = [ps.velocity[0], ps.velocity[1]];
        let hlen = (hvel[0] * hvel[0] + hvel[1] * hvel[1]).sqrt();
        let mut dir = if hlen > 20.0 {
            [hvel[0] / hlen, hvel[1] / hlen, 0.0]
        } else {
            // Use input direction (forward/strafe), else face forward.
            let mut d = [
                forward[0] * fwd_push + right[0] * side_push,
                forward[1] * fwd_push + right[1] * side_push,
                0.0,
            ];
            let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
            if l > 0.0 {
                d[0] /= l;
                d[1] /= l;
                d
            } else {
                [forward[0], forward[1], 0.0]
            }
        };

        // Also probe the opposite direction (a wall behind still lets you
        // wall-jump off it by pressing special into it). Prefer the movement
        // direction, then check both.
        let start = ps.origin;
        let mut tr = self.world.trace(
            start,
            crate::trace::PLAYER_MINS,
            crate::trace::PLAYER_MAXS,
            [start[0] + dir[0] * 32.0, start[1] + dir[1] * 32.0, start[2]],
        );
        if tr.fraction >= 1.0 {
            // Try the opposite horizontal direction.
            dir = [-dir[0], -dir[1], 0.0];
            tr = self.world.trace(
                start,
                crate::trace::PLAYER_MINS,
                crate::trace::PLAYER_MAXS,
                [start[0] + dir[0] * 32.0, start[1] + dir[1] * 32.0, start[2]],
            );
            if tr.fraction >= 1.0 {
                return; // no wall nearby
            }
        }

        let n = tr.normal;
        ps.special_held = true;

        // Wall-dash: reflect the horizontal velocity off the wall (keeping
        // magnitude), then ensure a minimum speed pushing away from it.
        let entry_speed =
            (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
        let dot = ps.velocity[0] * n[0] + ps.velocity[1] * n[1];
        let mut bounced = [
            ps.velocity[0] - 2.0 * dot * n[0],
            ps.velocity[1] - 2.0 * dot * n[1],
            ps.velocity[2],
        ];
        // Re-normalize the horizontal component to preserve entry speed (the
        // wall-dash redirects but does not bleed speed going straight on).
        let h_after =
            (bounced[0] * bounced[0] + bounced[1] * bounced[1]).sqrt();
        let target = entry_speed.max((WALK_SPEED + self.max_speed) * 0.5);
        if h_after > 1.0 {
            let s = target / h_after;
            bounced[0] *= s;
            bounced[1] *= s;
        } else {
            // Nearly stopped: push away along the wall normal.
            bounced[0] = -n[0] * target;
            bounced[1] = -n[1] * target;
        }
        if bounced[2] < PM_WJ_UPSPEED {
            bounced[2] = PM_WJ_UPSPEED;
        }

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
        // Apply ground friction (Quake PM_Friction). On slick surfaces (ice),
        // friction is skipped so the player slides.
        let slick = ps.ground_flags & SURF_SLICK != 0;
        if !slick {
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
            wishspeed = wishspeed.min(self.max_speed);
        }

        let accelerating = ps.velocity[0] * wishvel[0] + ps.velocity[1] * wishvel[1] > 0.0;

        if fwd != 0.0 && side == 0.0 && accelerating {
            // Forward-only (no strafe): forward-bunny — lets speed climb to
            // bunnytopspeed (Warfork PMFEAT_FWDBUNNY / PM_AirAccelerate).
            self.air_accelerate(ps, wishvel, wishspeed);
        } else if side != 0.0 && fwd == 0.0 {
            // Pure strafe (no forward): cap wishspeed and use bunny accel.
            if wishspeed > PM_WISHSPEED {
                wishspeed = PM_WISHSPEED;
            }
            self.accelerate(ps, wishvel, wishspeed, PM_STRANGE_BUNNY_ACCEL);
        } else {
            // Strafe-jump (forward + strafe): standard Quake air accel, which
            // produces forward momentum and speed-up at the strafe sweet spot.
            self.accelerate(ps, wishvel, wishspeed, PM_AIRACCELERATE);
        }

        // Air control: convert inertia toward the wish direction (Warfork
        // PM_Aircontrol, applies when moving, not strafing).
        if PM_AIRCONTROL != 0.0 && side == 0.0 && fwd != 0.0 {
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

    /// Slide the player along the world. First tries stair-stepping; otherwise
    /// performs Quake-style PM_SlideMove (multi-pass plane clipping).
    fn slide_move(&mut self, ps: &mut PlayerState) {
        let mins = crate::trace::PLAYER_MINS;
        let maxs = crate::trace::PLAYER_MAXS;

        let start = ps.origin;
        let end = [
            start[0] + ps.velocity[0] * self.frametime,
            start[1] + ps.velocity[1] * self.frametime,
            start[2] + ps.velocity[2] * self.frametime,
        ];

        let tr = self.world.trace(start, mins, maxs, end);
        if tr.fraction >= 1.0 {
            ps.origin = end;
            return;
        }

        // Blocked — try stair-stepping if moving mostly horizontally.
        let horizontal =
            (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
        if horizontal > 1.0 && self.try_step_up(ps, start, mins, maxs, end) {
            return;
        }

        self.slide_clip(ps, start, tr, mins, maxs);
    }

    /// Attempt an 18-unit stair-step (Quake PM_StepSlideMove). Returns true if
    /// a step was performed. Only fires when grounded and when the forward
    /// step actually makes meaningful progress (so running into a tall wall
    /// does not slowly "climb" it).
    fn try_step_up(
        &mut self,
        ps: &mut PlayerState,
        start: [f32; 3],
        mins: [f32; 3],
        maxs: [f32; 3],
        _end: [f32; 3],
    ) -> bool {
        // Only step when on the ground (not while pressed against a wall in
        // the air).
        if !ps.on_ground {
            return false;
        }

        let step = 18.0;
        let up_pos = [start[0], start[1], start[2] + step];
        let up_tr = self.world.trace(start, mins, maxs, up_pos);
        if up_tr.fraction < 1.0 {
            return false;
        }
        let fwd_end = [
            up_pos[0] + ps.velocity[0] * self.frametime,
            up_pos[1] + ps.velocity[1] * self.frametime,
            up_pos[2],
        ];
        let fwd_tr = self.world.trace(up_pos, mins, maxs, fwd_end);
        // Require meaningful forward progress (> 0.5 of the intended move),
        // otherwise we're just running into a tall wall.
        if fwd_tr.fraction <= 0.5 {
            return false;
        }
        let at = [
            up_pos[0] + (fwd_end[0] - up_pos[0]) * fwd_tr.fraction,
            up_pos[1] + (fwd_end[1] - up_pos[1]) * fwd_tr.fraction,
            up_pos[2],
        ];
        let down_end = [at[0], at[1], at[2] - step];
        let down_tr = self.world.trace(at, mins, maxs, down_end);
        // The landing must be no higher than a valid step (don't climb walls).
        let new_z = at[2] - step * down_tr.fraction;
        if new_z > start[2] + step + 1.0 {
            return false;
        }
        ps.origin = [at[0], at[1], new_z];
        if ps.velocity[2] < 0.0 {
            ps.velocity[2] = 0.0;
        }
        true
    }

    /// Quake PM_SlideMove: clip velocity against hit planes, re-tracing the
    /// leftover movement, up to MAX_CLIP_PLANES (5) passes, so the player
    /// slides along walls and into corners without sticking.
    #[allow(unused_assignments)]
    fn slide_clip(
        &mut self,
        ps: &mut PlayerState,
        start: [f32; 3],
        _first_tr: crate::trace::TraceResult,
        mins: [f32; 3],
        maxs: [f32; 3],
    ) {
        const MAX_CLIP_PLANES: usize = 5;
        let mut planes: [[f32; 3]; MAX_CLIP_PLANES] = [[0.0; 3]; MAX_CLIP_PLANES];
        let mut num_planes = 0;

        // Remaining displacement starts as the full tick's movement.
        let mut remaining = [
            ps.velocity[0] * self.frametime,
            ps.velocity[1] * self.frametime,
            ps.velocity[2] * self.frametime,
        ];
        let mut cur = start;

        for _pass in 0..MAX_CLIP_PLANES {
            let target = [cur[0] + remaining[0], cur[1] + remaining[1], cur[2] + remaining[2]];
            let tr = self.world.trace(cur, mins, maxs, target);

            if tr.fraction >= 1.0 {
                cur = target;
                break;
            }

            // Advance up to the impact + epsilon.
            let n = tr.normal;
            let eps = 0.05;
            cur = [
                cur[0] + remaining[0] * tr.fraction + n[0] * eps,
                cur[1] + remaining[1] * tr.fraction + n[1] * eps,
                cur[2] + remaining[2] * tr.fraction + n[2] * eps,
            ];

            // Reject planes already clipped against (avoid oscillation).
            let mut skip = false;
            for p in 0..num_planes {
                let d = planes[p][0] * n[0] + planes[p][1] * n[1] + planes[p][2] * n[2];
                if d > 0.99 {
                    skip = true;
                    break;
                }
            }
            if skip {
                break;
            }
            planes[num_planes] = n;
            num_planes += 1;

            // Clip the velocity against this plane.
            let dot = ps.velocity[0] * n[0] + ps.velocity[1] * n[1] + ps.velocity[2] * n[2];
            if dot < 0.0 {
                ps.velocity[0] -= n[0] * dot;
                ps.velocity[1] -= n[1] * dot;
                ps.velocity[2] -= n[2] * dot;
            }

            // Remaining movement uses the (clipped) velocity for the rest.
            remaining = [
                ps.velocity[0] * self.frametime * (1.0 - tr.fraction),
                ps.velocity[1] * self.frametime * (1.0 - tr.fraction),
                ps.velocity[2] * self.frametime * (1.0 - tr.fraction),
            ];
        }

        ps.origin = cur;
    }

    fn update_ground(&mut self, ps: &mut PlayerState) {
        ps.on_ground = self.grounded(ps);
    }
}

/// Convenience for tests: expose the crouch max speed.
pub const CROUCH_SPEEDV: f32 = CROUCH_SPEED;
