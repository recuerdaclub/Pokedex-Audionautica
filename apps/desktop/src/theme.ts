export const THEME_STORAGE_KEY = "audionautica-theme";

export const THEMES = [
  { id: "black", label: "Negro", swatch: "#111318" },
  { id: "neon", label: "Verde neon", swatch: "#c8f542" },
  { id: "ultra", label: "Ultravioleta", swatch: "#7c4dff" },
  { id: "red", label: "Rojo", swatch: "#ff3b4a" },
  { id: "pink", label: "Blanco", swatch: "#f4f6fa" },
] as const;

export type ThemeId = (typeof THEMES)[number]["id"];

export function isThemeId(value: string): value is ThemeId {
  return THEMES.some((theme) => theme.id === value);
}

export function readStoredTheme(): ThemeId {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    if (stored && isThemeId(stored)) return stored;
  } catch {
    /* ignore */
  }
  return "black";
}
