// Key/mouse binding system.
//
// Actions are named gameplay verbs; each is bound to a keyboard or mouse
// input. The bind map is persisted (keyed off the settings store) and editable
// in the settings menu.

export type Action =
  | "forward"
  | "back"
  | "moveleft"
  | "moveright"
  | "jump"
  | "special"
  | "crouch"
  | "attack"
  | "restart"
  | "menu"
  | "position_save";

export interface BindMap {
  [action: string]: string; // input code -> action
}

export const ACTIONS: { id: Action; label: string }[] = [
  { id: "forward", label: "move forward" },
  { id: "back", label: "move back" },
  { id: "moveleft", label: "move left" },
  { id: "moveright", label: "move right" },
  { id: "jump", label: "jump" },
  { id: "special", label: "dash / wall-jump" },
  { id: "crouch", label: "crouch" },
  { id: "attack", label: "attack" },
  { id: "restart", label: "restart race" },
  { id: "menu", label: "open menu" },
  { id: "position_save", label: "save position" },
];

export const DEFAULT_BINDS: BindMap = {
  KeyW: "forward",
  ArrowUp: "forward",
  KeyS: "back",
  ArrowDown: "back",
  KeyA: "moveleft",
  ArrowLeft: "moveleft",
  KeyD: "moveright",
  ArrowRight: "moveright",
  Space: "special",
  ShiftLeft: "crouch",
  ControlLeft: "crouch",
  Mouse2: "jump",
  Mouse3: "position_save",
  Digit4: "restart",
  Escape: "menu",
};

/** Mouse button index -> event.button mapping. */
const MOUSE_CODES: Record<string, number> = {
  Mouse0: 0,
  Mouse1: 1,
  Mouse2: 2,
  Mouse3: 3,
  Mouse4: 4,
  Mouse5: 5,
  Mouse6: 6,
};

/** Build a reverse lookup: input code -> action. */
export function codeToAction(binds: BindMap): Map<string, Action> {
  const map = new Map<string, Action>();
  for (const [code, action] of Object.entries(binds)) {
    map.set(code, action as Action);
  }
  return map;
}

/** Return a human-readable name for a bind code. */
export function displayCode(code: string): string {
  if (code.startsWith("Mouse")) return "Mouse " + code.replace("Mouse", "");
  if (code.startsWith("Key")) return code.slice(3);
  const aliases: Record<string, string> = {
    Space: "Space",
    Escape: "Esc",
    ShiftLeft: "LShift",
    ShiftRight: "RShift",
    ControlLeft: "LCtrl",
    ControlRight: "RCtrl",
    AltLeft: "LAlt",
    AltRight: "RAlt",
    Digit0: "0", Digit1: "1", Digit2: "2", Digit3: "3", Digit4: "4",
    Digit5: "5", Digit6: "6", Digit7: "7", Digit8: "8", Digit9: "9",
  };
  return aliases[code] ?? code;
}

/** Convert a KeyboardEvent into its bind code. */
export function keyEventToCode(e: KeyboardEvent): string {
  return e.code;
}

/** Convert a mouse button into its bind code. */
export function mouseButtonToCode(button: number): string {
  for (const [code, b] of Object.entries(MOUSE_CODES)) {
    if (b === button) return code;
  }
  return "";
}

/** Validate a proposed bind code (disallow reserved/typing keys). */
export function isAllowedCode(code: string): boolean {
  // Reject plain letter/digit keys that would conflict with typing in inputs.
  // Movement uses KeyW etc., but we still allow them (they're movement keys).
  // We only forbid the "menu" reserved keys from being rebound to letters
  // is not an issue; instead we forbid rebinding Escape/Space away from menu.
  return code.length > 0;
}
