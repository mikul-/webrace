// Movement HUD: acceleration bar + strafe-jump/bunny indicator triangle.
//
// The strafe triangle points at where you get the most acceleration (the
// optimal strafe angle), and grows when you're near it. You steer the mouse so
// the crosshair sits inside the triangle near its short base edge.

export class MovementHud {
  private accelFill: HTMLElement;
  private tri: SVGPolygonElement;
  private bunnyLine: SVGLineElement;

  constructor() {
    this.accelFill = document.getElementById("accel-fill")!;
    this.tri = document.getElementById("strafe-tri") as unknown as SVGPolygonElement;
    this.bunnyLine = document.getElementById("bunny-line") as unknown as SVGLineElement;
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
    const strafe = hint[9];

    // --- Acceleration bar ---
    const fill = 0.5 + Math.max(-0.5, Math.min(0.5, accel / 800));
    this.accelFill.style.width = `${(fill * 100).toFixed(1)}%`;
    this.accelFill.style.background =
      accel > 20 ? "#5ce27a" : accel < -20 ? "#e8483a" : "#ffb238";

    // --- Strafe triangle (points at max-accel sweet spot) ---
    const moving = speed > 30;
    const velLen = Math.hypot(vx, vy) + 1e-5;
    const vdx = vx / velLen;
    const vdy = vy / velLen;

    // Signed angle from velocity to wishdir.
    const dot = vdx * wx + vdy * wy;
    const cross = vdx * wy - vdy * wx;
    const theta = Math.atan2(cross, dot); // -PI..PI

    // Optimal strafe angle: wishdir perpendicular to velocity (theta = ±90°).
    // We want the angle error from the nearest ±90° sweet spot.
    // For a given strafe direction, the achievable sweet spot sign is fixed:
    //   strafe right (+1) -> you gain when turning so theta approaches +90°.
    //   strafe left  (-1) -> theta approaches -90°.
    const targetTheta = Math.sign(strafe || 1) * (Math.PI / 2);
    let err = targetTheta - theta;
    // wrap to -PI..PI
    err = Math.atan2(Math.sin(err), Math.cos(err));

    // How aligned (0 = perfect sweet spot, grows as you drift away).
    const errFrac = Math.abs(err) / (Math.PI / 2);

    // Triangle horizontal offset: pushed toward the sweet-spot direction.
    // +strafe right should place the triangle to the right of center when you
    // need to turn right. Map err -> lateral pixels.
    const shift = 74 * Math.sin(err);

    // Triangle scale: large + tall when at sweet spot (small err).
    const align = 1 - Math.min(1, errFrac);
    const hw = moving ? 10 + align * 32 : 10;
    const baseY = 52;
    const apexY = baseY - 8 - align * 28;

    const cx = 100 + shift;
    this.tri.setAttribute("points", `${cx},${apexY} ${cx + hw},${baseY} ${cx - hw},${baseY}`);
    this.tri.setAttribute("fill", moving ? "rgba(90,226,122,0.28)" : "rgba(90,226,122,0.06)");
    this.tri.setAttribute("stroke", moving ? "#5ce27a" : "#3a3f4a");

    // --- Bunny line (forward-bunny: tilt toward turn correction) ---
    const tilt = moving && Math.abs(strafe) < 0.01 ? Math.max(-16, Math.min(16, Math.sin(err) * 16)) : 0;
    this.bunnyLine.setAttribute("x1", String(100 - tilt));
    this.bunnyLine.setAttribute("x2", String(100 + tilt));
    this.bunnyLine.setAttribute("stroke", moving && Math.abs(strafe) < 0.01 && Math.abs(err) > 0.25 ? "#ffb238" : "#3a3f4a");
  }
}
