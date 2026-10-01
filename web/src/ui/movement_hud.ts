// Movement HUD: acceleration bar + strafe-jump/bunny indicator triangle.
//
// The strafe triangle follows the Warsow/Warfork "speedometer key hint":
//   - it moves left/right as your view angle drifts from the optimal strafe
//     angle (so you steer the crosshair to chase it),
//   - it grows when you're gaining speed well and shrinks when you're not,
//   - you keep your crosshair near the triangle's short base edge, inside it.

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
   * Update from the sim hint (8 floats):
   *   vx, vy (velocity), 0, wx, wy (wishdir), 0, speed, accel
   */
  update(hint: Float32Array | number[]) {
    const vx = hint[0];
    const vy = hint[1];
    const wx = hint[3];
    const wy = hint[4];
    const speed = hint[6];
    const accel = hint[7];

    // --- Acceleration bar (fill + color) ---
    // dot(velocity, wishdir) > 0 = gaining. Normalize around a reference gain.
    const fill = 0.5 + Math.max(-0.5, Math.min(0.5, accel / 800));
    this.accelFill.style.width = `${(fill * 100).toFixed(1)}%`;
    this.accelFill.style.background =
      accel > 20 ? "#5ce27a" : accel < -20 ? "#e8483a" : "#ffb238";

    // --- Strafe triangle ---
    // Turn error: signed cross(velocity, wishdir). Sign gives the side you're
    // misaligned toward; magnitude is the alignment error (radian-scale).
    const cross = vx * wy - vy * wx;
    // Alignment: how perpendicular velocity is to wish (strafe sweet spot is
    // ~90°, dot -> 0). Quality peaks near perpendicular.
    const perp = Math.abs(cross) / (speed + 1e-3); // sin(angle) ~ 0..1
    const alignQuality = perp; // 1 = most perpendicular (good strafe), 0 = aligned

    const moving = speed > 30;

    // Triangle horizontal offset (follow the turn error). +cross = shift right.
    const shift = Math.max(-70, Math.min(70, cross * 0.08));

    // Triangle size: base half-width scales with alignment quality + speed.
    const best = 34; // half-width at perfect alignment
    const minW = 10;
    const hw = moving
      ? minW + alignQuality * best
      : minW;

    const cx = 100 + shift;
    const baseY = 52;
    const apexY = baseY - 8 - alignQuality * 28; // taller triangle = more gain

    this.tri.setAttribute(
      "points",
      `${cx},${apexY} ${cx + hw},${baseY} ${cx - hw},${baseY}`,
    );
    this.tri.setAttribute(
      "fill",
      moving ? "rgba(90,226,122,0.28)" : "rgba(90,226,122,0.06)",
    );
    this.tri.setAttribute("stroke", moving ? "#5ce27a" : "#3a3f4a");

    // --- Bunny line (forward-bunny turn correction, tilt left/right) ---
    const tilt = moving ? Math.max(-16, Math.min(16, cross * 0.03)) : 0;
    this.bunnyLine.setAttribute("x1", String(100 - tilt));
    this.bunnyLine.setAttribute("x2", String(100 + tilt));
    this.bunnyLine.setAttribute(
      "stroke",
      moving && Math.abs(cross) > 40 ? "#ffb238" : "#3a3f4a",
    );
  }
}
