// Movement HUD: renders the acceleration bar + strafe/bunny turn indicators
// from the simulation's movement hint (velocity, wish direction, speed, accel).

export class MovementHud {
  private accelFill: HTMLElement;
  private strafeLeft: SVGPolygonElement;
  private strafeRight: SVGPolygonElement;
  private bunnyLine: SVGLineElement;

  constructor() {
    this.accelFill = document.getElementById("accel-fill")!;
    this.strafeLeft = document.getElementById("strafe-left") as unknown as SVGPolygonElement;
    this.strafeRight = document.getElementById("strafe-right") as unknown as SVGPolygonElement;
    this.bunnyLine = document.getElementById("bunny-line") as unknown as SVGLineElement;
  }

  /**
   * Update from the sim hint (8 floats):
   *   vx, vy (=velocity), 0, wx, wy (=wish dir), 0, speed, accel
   */
  update(hint: Float32Array | number[]) {
    const vx = hint[0];
    const vy = hint[1];
    const wx = hint[3];
    const wy = hint[4];
    const speed = hint[6];
    const accel = hint[7];

    // Clamp speed to a display range (0..~1000 ups).
    void speed;

    // --- Acceleration bar ---
    // accel is dot(velocity, wishdir); normalize to a 0..1 fill where 0.5 is
    // "no gain", 1.0 is "gaining hard", 0.0 is "losing hard".
    // Use a reference of ~640 ups for full-scale gain.
    let fill = 0.5;
    const gain = accel / 800; // full bar at ±800
    fill = 0.5 + Math.max(-0.5, Math.min(0.5, gain));

    this.accelFill.style.width = `${(fill * 100).toFixed(1)}%`;
    this.accelFill.style.background =
      accel > 20 ? "#5ce27a" : accel < -20 ? "#e8483a" : "#ffb238";

    // --- Strafe triangles + bunny line ---
    // Turn direction: cross(velocity, wishdir) = vx*wy - vy*wx.
    // Positive -> turn left; negative -> turn right. Magnitude ~ misalignment.
    const cross = vx * wy - vy * wx;
    const alive = speed > 30;

    const leftOn = alive && cross > 4;
    const rightOn = alive && cross < -4;

    this.strafeLeft.setAttribute(
      "fill",
      leftOn ? "#5ce27a" : "#3a3f4a",
    );
    this.strafeRight.setAttribute(
      "fill",
      rightOn ? "#5ce27a" : "#3a3f4a",
    );

    // Bunny line: tilt left/right to indicate forward-bunny turn correction.
    // When not strafing and airborne, the line leans toward the needed turn.
    const tilt = alive ? Math.max(-14, Math.min(14, cross * 0.02)) : 0;
    this.bunnyLine.setAttribute("x1", String(90 - tilt));
    this.bunnyLine.setAttribute("x2", String(90 + tilt));
    this.bunnyLine.setAttribute(
      "stroke",
      Math.abs(cross) > 50 && alive ? "#ffb238" : "#3a3f4a",
    );
  }
}
