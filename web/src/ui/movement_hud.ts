// Movement HUD: acceleration bar + strafe-jump/bunny-hop helpers.
//
// - Strafe-jump (W + A/D): triangle above the crosshair points at where to move
//   the mouse to gain the most strafe speed.
// - Bunny-hop (only A/D): a second marker showing where to aim to keep
//   forward-bunny acceleration. Its zone shrinks as speed rises.

export class MovementHud {
  private accelFill: HTMLElement;
  private strafeTri: SVGPolygonElement;
  private bunnyTri: SVGPolygonElement;

  constructor() {
    this.accelFill = document.getElementById("accel-fill")!;
    this.strafeTri = document.getElementById("strafe-tri") as unknown as SVGPolygonElement;
    this.bunnyTri = document.getElementById("bunny-tri") as unknown as SVGPolygonElement;
  }

  /**
   * Update from the sim hint (10 floats):
   *   vx, vy, 0 (velocity), wx, wy, 0 (wishdir), speed, accel, fwd, strafe
   */
  update(hint: Float32Array | number[]) {
    const vx = hint[0];
    const vy = hint[1];
    const wx = hint[3];
    const wy = hint[4];
    const speed = hint[6];
    const accel = hint[7];
    const fwd = hint[8];
    const strafe = hint[9];

    // --- Acceleration bar ---
    const fill = 0.5 + Math.max(-0.5, Math.min(0.5, accel / 800));
    this.accelFill.style.width = `${(fill * 100).toFixed(1)}%`;
    this.accelFill.style.background =
      accel > 20 ? "#5ce27a" : accel < -20 ? "#e8483a" : "#ffb238";

    const moving = speed > 30;
    const velLen = Math.hypot(vx, vy) + 1e-5;
    const vdx = vx / velLen;
    const vdy = vy / velLen;

    // Angle from velocity to wishdir.
    const dot = vdx * wx + vdy * wy;
    const cross = vdx * wy - vdy * wx;
    const theta = Math.atan2(cross, dot);

    // Acceleration window narrows with speed: higher speed -> tighter sweet spot.
    const speedWindow = Math.max(0.12, 1 - speed / 900); // 1 (slow) -> ~0.12 (fast)

    const strafing = Math.abs(strafe) > 0.05;
    const bunnyOnly = !strafing && Math.abs(fwd) < 0.05;

    if (strafing) {
      // Strafe-jump: target theta = ±90° (wishdir perpendicular to velocity),
      // matching the held strafe key. Show the triangle at the sweet-spot
      // direction; keep the crosshair inside its base.
      const target = Math.sign(strafe) * (Math.PI / 2);
      let err = target - theta;
      err = Math.atan2(Math.sin(err), Math.cos(err));
      const errFrac = Math.abs(err) / (Math.PI / 2);

      const shift = 74 * Math.sin(err);
      const align = 1 - Math.min(1, errFrac / speedWindow);
      const hw = 10 + Math.max(0, align) * 32;
      const baseY = 36;
      const apexY = baseY - 6 - Math.max(0, align) * 26;
      const cx = 100 + shift;

      this.strafeTri.setAttribute("points", `${cx},${apexY} ${cx + hw},${baseY} ${cx - hw},${baseY}`);
      this.strafeTri.setAttribute("fill", "rgba(90,226,122,0.28)");
      this.strafeTri.setAttribute("stroke", "#5ce27a");
      this.strafeTri.setAttribute("opacity", "1");
      this.bunnyTri.setAttribute("opacity", "0");
    } else if (bunnyOnly) {
      // Bunny-hop (A/D only): show where to aim; zone shrinks with speed.
      // The forward-bunny sweet spot is velocity roughly aligned with wishdir,
      // so the error is how far velocity is from the wish direction.
      let err = theta;
      err = Math.atan2(Math.sin(err), Math.cos(err));

      const shift = 74 * Math.sin(err);
      const align = 1 - Math.min(1, Math.abs(err) / (Math.PI / 2) / speedWindow);
      const hw = 10 + Math.max(0, align) * 32;
      const baseY = 18;
      const apexY = baseY - 4 - Math.max(0, align) * 12;
      const cx = 100 + shift;

      this.bunnyTri.setAttribute("points", `${cx},${apexY} ${cx + hw},${baseY} ${cx - hw},${baseY}`);
      this.bunnyTri.setAttribute("fill", "rgba(255,178,56,0.28)");
      this.bunnyTri.setAttribute("stroke", "#ffb238");
      this.bunnyTri.setAttribute("opacity", moving ? "1" : "0.3");
      this.strafeTri.setAttribute("opacity", "0");
    } else {
      // Not moving in a relevant way: hide both.
      this.strafeTri.setAttribute("opacity", "0");
      this.bunnyTri.setAttribute("opacity", "0");
    }
  }
}
