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
    /// Whether the player is currently crouched (shrinks the box + view).
    pub crouched: bool,
    /// Surface flags of the ground currently stood on (SURF_SLICK etc.).
    pub ground_flags: i32,
    /// Normal of the ground plane currently stood on (for slope sliding).
    pub ground_normal: [f32; 3],
    /// Water submersion level (0 none, 1 feet, 2 waist, 3 head).
    pub waterlevel: i32,
    /// Contents of the liquid the player is in (CONTENTS_WATER/LAVA/SLIME).
    pub watertype: i32,
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
            crouched: false,
            ground_flags: 0,
            ground_normal: [0.0, 0.0, 1.0],
            waterlevel: 0,
            watertype: 0,
            speed: 0.0,
        }
    }
}

/// Surface flags (mirrors QFusion qfiles.h).
pub const SURF_SLICK: i32 = 0x2;

/// Water movement constants (Warfork `gs_pmove.c`).
pub const PM_WATERACCELERATE: f32 = 10.0;
pub const PM_WATERFRICTION: f32 = 1.0;

/// Step height used by `PM_StepSlideMove` (Q3 `STEPSIZE`, Warfork too).
pub const STEPSIZE: f32 = 18.0;
/// Q3 `OVERCLIP`: clip velocity against surfaces with a slight overbounce to
/// keep the player glued to the ground/walls without losing speed on stairs.
pub const OVERCLIP: f32 = 1.001;
/// Q3 `MIN_STEP_NORMAL`: a plane steeper than this is not walkable.
pub const MIN_STEP_NORMAL: f32 = 0.7;

pub struct Pmove {
    pub world: World,
    pub frametime: f32,
    pub max_speed: f32,
    pub max_player_speed: f32,
    /// Current player box (updated each tick from crouch state).
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}

impl Pmove {
    pub fn new(world: World, frametime: f32) -> Self {
        // `max_speed` and `max_player_speed` are both the ground run speed
        // (Warfork `DEFAULT_PLAYERSPEED` = 320 ups, shared by race/instagib/
        // standard). Warfork has a single `maxPlayerSpeed` that grounds both
        // the ground speed cap and the forward-bunny reference speed — there is
        // no separate higher "air-bunny" reference (that was a mis-port that
        // made a forward jump ramp to ~600 instead of ~350).
        Pmove {
            world,
            frametime,
            max_speed: 320.0,
            max_player_speed: 320.0,
            mins: crate::trace::PLAYER_MINS,
            maxs: crate::trace::PLAYER_MAXS,
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

        // Down trace tolerance for ground contact. Larger than Q3's 0.25 because
        // our slide/step does not carry a persistent ground plane, so a small
        // margin keeps the player grounded while descending ramps/stairs.
        let down = 2.0;
        let start = ps.origin;
        let end = [ps.origin[0], ps.origin[1], ps.origin[2] - down];
        let tr = self.world.trace(
            start,
            self.mins,
            self.maxs,
            end,
        );
        // Grounded only if we actually hit something walkable (flat enough).
        if tr.fraction >= 1.0 {
            ps.ground_flags = 0;
            ps.ground_normal = [0.0, 0.0, 1.0];
            return false;
        }
        // Warfork `ISWALKABLEPLANE`: normal.z >= 0.7 for ALL surfaces (slick or
        // not). Steeper surfaces are NOT ground — the player is airborne there
        // and gravity slides them down, which is how steep slick ramps
        // accelerate. (A start-solid surface still counts as ground, matching
        // `PM_CategorizePosition`.)
        if tr.normal[2] >= MIN_STEP_NORMAL || tr.start_solid {
            ps.ground_flags = tr.surface_flags;
            ps.ground_normal = tr.normal;
            return true;
        }
        ps.ground_flags = 0;
        ps.ground_normal = [0.0, 0.0, 1.0];
        false
    }

    /// Detect the player's submersion level (0..3) and liquid type, sampling
    /// point-contents at three heights (feet+1, waist, head). Mirrors Q3
    /// `PM_WaterMove` entry / `Pmove_WaterLevel`.
    fn update_water(&mut self, ps: &mut PlayerState) {
        ps.waterlevel = 0;
        ps.watertype = 0;

        let view_height = if ps.crouched { 18.0 } else { 26.0 };
        // Relative sample heights above the box origin (feet at origin[2]-24).
        let feet = ps.origin[2] - 24.0 + 1.0;
        let mid = ps.origin[2] - 24.0 + view_height * 0.5;
        let head = ps.origin[2] - 24.0 + view_height;

        let c = |z: f32| self.world.point_contents([ps.origin[0], ps.origin[1], z]);
        const MASK_WATER: i32 = crate::bsp::CONTENTS_WATER
            | crate::bsp::CONTENTS_LAVA
            | crate::bsp::CONTENTS_SLIME;

        let feet_c = c(feet);
        if feet_c & MASK_WATER != 0 {
            ps.watertype = feet_c & MASK_WATER;
            ps.waterlevel = 1;
            let mid_c = c(mid);
            if mid_c & MASK_WATER != 0 {
                ps.waterlevel = 2;
                let head_c = c(head);
                if head_c & MASK_WATER != 0 {
                    ps.waterlevel = 3;
                }
            }
        }
    }

    /// Q3 `PM_WaterMove`: swimming. Build a wish direction from the view-space
    /// movement keys plus vertical input (drift down when idle), clamp, and
    /// accelerate with `pm_wateraccelerate`.
    fn water_move(
        &mut self,
        ps: &mut PlayerState,
        forward: [f32; 3],
        right: [f32; 3],
        fwd_push: f32,
        side_push: f32,
        up_push: f32,
    ) {
        let mut wishvel = [
            forward[0] * fwd_push + right[0] * side_push,
            forward[1] * fwd_push + right[1] * side_push,
            forward[2] * fwd_push + right[2] * side_push,
        ];

        if fwd_push == 0.0 && side_push == 0.0 && up_push == 0.0 {
            wishvel[2] -= 60.0; // drift toward the bottom
        } else {
            wishvel[2] += up_push;
        }

        let mut wishdir = wishvel;
        let mut wishspeed = (wishdir[0] * wishdir[0] + wishdir[1] * wishdir[1] + wishdir[2] * wishdir[2]).sqrt();
        if wishspeed > 1e-6 {
            wishdir[0] /= wishspeed;
            wishdir[1] /= wishspeed;
            wishdir[2] /= wishspeed;
        }
        // Clamp to max swim speed (Q3 scales wishspeed to max_speed).
        if wishspeed > self.max_speed {
            wishspeed = self.max_speed;
        }
        wishspeed *= 0.5;

        self.accelerate(ps, wishdir, wishspeed, PM_WATERACCELERATE);
    }

    /// Q3 water drag: `drop += speed * pm_waterfriction * waterlevel * frametime`.
    fn water_friction(&mut self, ps: &mut PlayerState) {
        let speed = (ps.velocity[0] * ps.velocity[0]
            + ps.velocity[1] * ps.velocity[1]
            + ps.velocity[2] * ps.velocity[2])
            .sqrt();
        if speed < 1.0 {
            ps.velocity = [0.0, 0.0, 0.0];
            return;
        }
        let drop = speed * PM_WATERFRICTION * ps.waterlevel as f32 * self.frametime;
        let newspeed = (speed - drop).max(0.0) / speed.max(0.0001);
        ps.velocity[0] *= newspeed;
        ps.velocity[1] *= newspeed;
        ps.velocity[2] *= newspeed;
    }

    /// Advance one tick.
    pub fn step(&mut self, ps: &mut PlayerState, cmd: &Cmd) {
        let special = cmd.buttons & crate::input::BUTTON_SPECIAL != 0;
        let jump = cmd.buttons & crate::input::BUTTON_JUMP != 0;
        let crouch = cmd.buttons & crate::input::BUTTON_CROUCH != 0;

        // Crouch state. (We don't yet block uncrouching under low ceilings.)
        ps.crouched = crouch;
        self.mins = if crouch { crate::trace::CROUCH_MINS } else { crate::trace::PLAYER_MINS };
        self.maxs = if crouch { crate::trace::CROUCH_MAXS } else { crate::trace::PLAYER_MAXS };

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

        // Detect submersion (waterlevel 0..3) at three sample heights, like
        // Q3 `PM_WaterMove` entry. Sample at feet+1, +half, +full view height.
        self.update_water(ps);

        if ps.waterlevel >= 2 {
            // Swimming: water move replaces ground/air movement; jump does not
            // fire (Q3 `PM_CheckJump` returns when waterlevel >= 2).
            self.water_move(ps, forward, right, fwd_push, side_push, up_push);
            // Apply water friction + integrate.
            self.water_friction(ps);
            self.slide_move(ps);
            ps.speed = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
            self.update_ground(ps);
            self.check_triggers(ps);
            // Keep the player out of the ceiling while swimming.
            let _ = up;
            return;
        }

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
            if ps.velocity[2] > 0.0 {
                ps.velocity[2] = 0.0; // Warfork zeroes upward velocity on ground
            }
            self.ground_move(ps, forward, right, fwd_push, side_push);
        } else {
            self.air_move(ps, forward, right, fwd_push, side_push);
        }

        // Gravity: only in the air, exactly like Warfork. On the ground (slick
        // or not) there is no downward gravity; ramp descent comes from the
        // step-down in `slide_move` (which adopts the down-slide's z velocity).
        if !ps.on_ground {
            ps.velocity[2] -= GRAVITY * self.frametime;
        }

        // Ground-plane projection: keep the player on the surface by clipping
        // the velocity against the ground normal. On a descending slope this
        // redirects horizontal motion into downhill motion.
        if ps.on_ground {
            let n = ps.ground_normal;
            if n[2] > 0.001 {
                let dot = ps.velocity[0] * n[0] + ps.velocity[1] * n[1] + ps.velocity[2] * n[2];
                if dot < 0.0 {
                    ps.velocity[0] -= n[0] * dot;
                    ps.velocity[1] -= n[1] * dot;
                    ps.velocity[2] -= n[2] * dot;
                }
            }
        }

        // Integrate position with collision.
        self.slide_move(ps);

        // Horizontal speed only (ups = forward/strafe speed, not vertical).
        ps.speed = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();

        // Recompute ground contact.
        self.update_ground(ps);

        // Triggers: teleporters move the player; jumppads launch them.
        self.check_triggers(ps);

        // Anti-float: if grounded, keep the feet pinned to the ground plane so
        // the slide/epsilon at angled seams cannot drift the player upward.
        // (Load-bearing for smooth ramp/stairs traversal.)
        if ps.on_ground {
            let start = ps.origin;
            let end = [start[0], start[1], start[2] - 30.0];
            let tr = self.world.trace(start, self.mins, self.maxs, end);
            if tr.fraction < 1.0 && tr.normal[2] > 0.7 {
                let surface_z = start[2] - 30.0 * tr.fraction;
                let target_z = surface_z + 24.0;
                if ps.origin[2] > target_z {
                    ps.origin[2] = target_z;
                    if ps.velocity[2] < 0.0 {
                        ps.velocity[2] = 0.0;
                    }
                }
            }
        }
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
        let target = hspeed.max(crate::DASH_SPEED);
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
        // Crouching caps the ground move speed (Warfork crouch speed).
        let speed_cap = if ps.crouched { crate::CROUCH_SPEED } else { self.max_speed };
        wishspeed = wishspeed.min(speed_cap);

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
        // `VectorNormalize` in C normalizes in place AND returns the length.
        let speed = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1])
            .sqrt();
        if speed == 0.0 {
            ps.velocity[2] = zspeed;
            return;
        }
        // Normalize the horizontal velocity direction (unit vector), matching
        // the C `VectorNormalize` side effect.
        let vx = ps.velocity[0] / speed;
        let vy = ps.velocity[1] / speed;
        let dot = vx * wishdir[0] + vy * wishdir[1];
        let k = 32.0 * PM_AIRCONTROL * dot * dot * self.frametime;
        if dot > 0.0 {
            // vx/vy are unit-length, so `unit * speed` restores magnitude.
            ps.velocity[0] = vx * speed + wishdir[0] * k;
            ps.velocity[1] = vy * speed + wishdir[1] * k;
            let ns = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1])
                .sqrt();
            if ns > 0.0 {
                ps.velocity[0] = ps.velocity[0] / ns * speed;
                ps.velocity[1] = ps.velocity[1] / ns * speed;
            }
        }
        ps.velocity[2] = zspeed;
    }

    /// Slide the player along the world, faithfully porting Warfork
    /// `PM_StepSlideMove`: do a plain `PM_SlideMove`, then retry the same move
    /// from `STEPSIZE` higher and keep whichever advanced farther horizontally.
    /// On a walkable ramp the horizontal speed is preserved and the vertical
    /// velocity adopts the down-slide's z — this is what produces ramp sliding
    /// (there is no gravity applied on the ground).
    fn slide_move(&mut self, ps: &mut PlayerState) {
        let mins = self.mins;
        let maxs = self.maxs;

        let start_o = ps.origin;
        let start_v = ps.velocity;

        // Warfork `PM_SlideMove`.
        self.slide_clip(ps, mins, maxs);

        // Save the plain-slide result (the "down" candidate).
        let down_o = ps.origin;
        let down_v = ps.velocity;

        // Try the same move from STEPSIZE higher. Warfork checks the raised
        // position with a zero-length trace (allsolid => can't step).
        let up = [start_o[0], start_o[1], start_o[2] + STEPSIZE];
        let tr = self.world.trace(up, mins, maxs, up);
        if tr.all_solid {
            return; // can't step up
        }

        ps.origin = up;
        ps.velocity = start_v;
        self.slide_clip(ps, mins, maxs);

        // Push down the final amount.
        let down = [ps.origin[0], ps.origin[1], ps.origin[2] - STEPSIZE];
        let tr = self.world.trace(ps.origin, mins, maxs, down);
        if !tr.all_solid {
            ps.origin = [
                ps.origin[0] + (down[0] - ps.origin[0]) * tr.fraction,
                ps.origin[1] + (down[1] - ps.origin[1]) * tr.fraction,
                ps.origin[2] + (down[2] - ps.origin[2]) * tr.fraction,
            ];
        }

        // Decide which one went farther horizontally.
        let down_dist = (down_o[0] - start_o[0]).powi(2) + (down_o[1] - start_o[1]).powi(2);
        let up_dist = (ps.origin[0] - start_o[0]).powi(2) + (ps.origin[1] - start_o[1]).powi(2);
        if down_dist >= up_dist
            || tr.all_solid
            || (tr.fraction != 1.0 && tr.normal[2] < MIN_STEP_NORMAL)
        {
            ps.origin = down_o;
            ps.velocity = down_v;
            return;
        }

        // Preserve speed when sliding up ramps (Warfork PM_StepSlideMove).
        let hspeed = (start_v[0] * start_v[0] + start_v[1] * start_v[1]).sqrt();
        if hspeed > 0.0 && tr.normal[2] >= MIN_STEP_NORMAL {
            if tr.normal[2] >= 1.0 - 0.05 {
                ps.velocity = start_v;
            } else {
                let h = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1])
                    .sqrt();
                if h > 0.0 {
                    ps.velocity[0] *= hspeed / h;
                    ps.velocity[1] *= hspeed / h;
                }
            }
        }

        // wsw/jal: the following line produces the ramp sliding.
        ps.velocity[2] = down_v[2];

        // Final safety: if the slide left the player embedded, unstick them.
        let chk = self.world.trace(ps.origin, mins, maxs, ps.origin);
        if chk.start_solid {
            self.resolve_solid(ps, ps.origin, mins, maxs);
        }
    }

    /// Push the player out of geometry they are embedded in, scanning the
    /// 6 axis directions (+/-x, +/-y, +/-z) for the smallest push that clears,
    /// preferring the shallow horizontal axes first. Leaves `origin` unchanged
    /// if no direction clears (shouldn't happen in practice).
    fn resolve_solid(
        &mut self,
        ps: &mut PlayerState,
        from: [f32; 3],
        mins: [f32; 3],
        maxs: [f32; 3],
    ) {
        let dirs: [[f32; 3]; 6] = [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ];
        let steps: [f32; 8] = [1.0, 2.0, 4.0, 8.0, 16.0, 24.0, 32.0, 48.0];
        for d in dirs {
            for s in steps {
                let cand = [from[0] + d[0] * s, from[1] + d[1] * s, from[2] + d[2] * s];
                if !self.world.trace(cand, mins, maxs, cand).start_solid {
                    ps.origin = cand;
                    return;
                }
            }
        }
        // No direction cleared (fully embedded); leave as-is.
    }

    /// Faithful port of Warfork `PM_SlideMove`: move along the world, clipping
    /// the velocity against up to `MAX_CLIP_PLANES` planes (walls, floors,
    /// creases) using `PM_OVERBOUNCE` (1.01). Does not pre-seed the ground
    /// plane; instead it zeroes downward velocity when standing on a flat floor.
    fn slide_clip(&mut self, ps: &mut PlayerState, mins: [f32; 3], maxs: [f32; 3]) {
        const MAX_CLIP_PLANES: usize = 5;
        const PLANE_EPS: f32 = 0.05;
        const OVERBOUNCE: f32 = 1.01;
        let maxmoves = 4;

        let mut last_valid_origin = ps.origin;

        // If standing on flat ground, don't let downward velocity build up.
        if ps.on_ground && ps.ground_normal[2] == 1.0 && ps.velocity[2] < 0.0 {
            ps.velocity[2] = 0.0;
        }

        let mut planes: [[f32; 3]; MAX_CLIP_PLANES] = [[0.0; 3]; MAX_CLIP_PLANES];
        let mut numplanes = 0usize;
        let mut remaining_time = self.frametime;

        for _move in 0..maxmoves {
            let end = [
                ps.origin[0] + remaining_time * ps.velocity[0],
                ps.origin[1] + remaining_time * ps.velocity[1],
                ps.origin[2] + remaining_time * ps.velocity[2],
            ];
            let tr = self.world.trace(ps.origin, mins, maxs, end);

            if tr.all_solid || tr.start_solid {
                // Trapped/embedded — restore the last valid position, unstick.
                ps.origin = last_valid_origin;
                if tr.start_solid {
                    self.resolve_solid(ps, ps.origin, mins, maxs);
                }
                return;
            }

            if tr.fraction > 0.0 {
                ps.origin = [
                    ps.origin[0] + (end[0] - ps.origin[0]) * tr.fraction,
                    ps.origin[1] + (end[1] - ps.origin[1]) * tr.fraction,
                    ps.origin[2] + (end[2] - ps.origin[2]) * tr.fraction,
                ];
                last_valid_origin = ps.origin;
            }

            if tr.fraction == 1.0 {
                break; // move done
            }

            remaining_time -= tr.fraction * remaining_time;

            // Repeated plane: nudge velocity along its normal and retry.
            let normal = tr.normal;
            let mut repeated = false;
            for p in 0..numplanes {
                if planes[p][0] * normal[0] + planes[p][1] * normal[1] + planes[p][2] * normal[2]
                    > 1.0 - PLANE_EPS
                {
                    ps.velocity[0] += normal[0];
                    ps.velocity[1] += normal[1];
                    ps.velocity[2] += normal[2];
                    repeated = true;
                    break;
                }
            }
            if repeated {
                continue;
            }

            if numplanes >= MAX_CLIP_PLANES {
                ps.velocity = [0.0, 0.0, 0.0];
                return;
            }

            planes[numplanes] = normal;
            numplanes += 1;

            // Modify velocity so it parallels all clip planes.
            for i in 0..numplanes {
                let into = ps.velocity[0] * planes[i][0]
                    + ps.velocity[1] * planes[i][1]
                    + ps.velocity[2] * planes[i][2];
                if into >= PLANE_EPS {
                    continue; // move doesn't interact with this plane
                }
                clip_velocity(&mut ps.velocity, planes[i], OVERBOUNCE);

                for j in 0..numplanes {
                    if j == i {
                        continue;
                    }
                    let into_j = ps.velocity[0] * planes[j][0]
                        + ps.velocity[1] * planes[j][1]
                        + ps.velocity[2] * planes[j][2];
                    if into_j >= PLANE_EPS {
                        continue;
                    }
                    clip_velocity(&mut ps.velocity, planes[j], OVERBOUNCE);

                    let back = ps.velocity[0] * planes[i][0]
                        + ps.velocity[1] * planes[i][1]
                        + ps.velocity[2] * planes[i][2];
                    if back >= PLANE_EPS {
                        continue;
                    }

                    // Slide the original velocity along the crease.
                    let mut dir = cross(planes[i], planes[j]);
                    let dl = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                    if dl > 0.0 {
                        dir[0] /= dl;
                        dir[1] /= dl;
                        dir[2] /= dl;
                    }
                    let val = dir[0] * ps.velocity[0] + dir[1] * ps.velocity[1] + dir[2] * ps.velocity[2];
                    ps.velocity = [dir[0] * val, dir[1] * val, dir[2] * val];

                    // Third plane interacting: trapped.
                    for k in 0..numplanes {
                        if k == i || k == j {
                            continue;
                        }
                        let into_k = ps.velocity[0] * planes[k][0]
                            + ps.velocity[1] * planes[k][1]
                            + ps.velocity[2] * planes[k][2];
                        if into_k >= PLANE_EPS {
                            continue;
                        }
                        ps.velocity = [0.0, 0.0, 0.0];
                        break;
                    }
                }
            }
        }
    }

    fn update_ground(&mut self, ps: &mut PlayerState) {
        ps.on_ground = self.grounded(ps);
    }

    /// Trigger overlap handling: teleporters move the player (preserving
    /// velocity + horizontal yaw), jumppads replace velocity with a launch.
    fn check_triggers(&mut self, ps: &mut PlayerState) {
        let mins = self.mins;
        let maxs = self.maxs;
        let origin = ps.origin;

        if let Some(tp) = self.world.teleporter_at(origin, mins, maxs) {
            let dest = tp.dest_origin;
            ps.origin = dest;
            self.drop_to_ground(ps);
            return;
        }

        if let Some(jp) = self.world.jumppad_at(origin, mins, maxs) {
            ps.velocity = jp.velocity;
            ps.on_ground = false;
            ps.wjtime = 0;
            ps.doshtime = 0;
        }
    }
}

/// Convenience for tests: expose the crouch max speed.
pub const CROUCH_SPEEDV: f32 = CROUCH_SPEED;

/// Q3 `PM_ClipVelocity`: slide a velocity off a surface normal with a slight
/// overbounce (or underbounce, depending on direction).
fn clip_velocity(vel: &mut [f32; 3], normal: [f32; 3], overbounce: f32) {
    let mut backoff = vel[0] * normal[0] + vel[1] * normal[1] + vel[2] * normal[2];
    if backoff < 0.0 {
        backoff *= overbounce;
    } else {
        backoff /= overbounce;
    }
    vel[0] -= normal[0] * backoff;
    vel[1] -= normal[1] * backoff;
    vel[2] -= normal[2] * backoff;
}

/// Cross product of two vectors (for crease sliding).
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
