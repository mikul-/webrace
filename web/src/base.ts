// Base path helper. In production the app is served under a subdirectory
// (Vite `base`), e.g. `/webrace/`; in dev it's the root. `import.meta.env.BASE_URL`
// reflects the Vite `base` setting automatically.

export const BASE = import.meta.env.BASE_URL;

/** Prefix a server API/asset path with the app base. */
export function api(path: string): string {
  return BASE + path.replace(/^\//, "");
}
