/**
 * Bubble palette — sets CSS custom properties used by ChatMessage to color the
 * glow shadow behind user/assistant bubbles. Persists to localStorage.
 */

export type PaletteId =
  | "indigo-orange"
  | "teal-magenta"
  | "mono"
  | "forest"
  | "sunset"
  | "ocean"
  | "amethyst"
  | "midnight"
  | "ember"
  | "mint";

export interface PalettePreset {
  id: PaletteId;
  label: string;
  // Each color is an "r g b" triplet so we can set alpha at use site.
  user: string;
  assistant: string;
}

export const PALETTE_PRESETS: PalettePreset[] = [
  { id: "indigo-orange", label: "Indigo / Orange", user: "99 102 241", assistant: "249 115 22" },
  { id: "teal-magenta", label: "Teal / Magenta", user: "20 184 166", assistant: "236 72 153" },
  { id: "ocean", label: "Ocean", user: "56 189 248", assistant: "251 113 133" },
  { id: "amethyst", label: "Amethyst", user: "168 85 247", assistant: "245 158 11" },
  { id: "midnight", label: "Midnight", user: "71 85 105", assistant: "34 211 238" },
  { id: "ember", label: "Ember", user: "220 38 38", assistant: "250 204 21" },
  { id: "mint", label: "Mint", user: "16 185 129", assistant: "217 70 239" },
  { id: "forest", label: "Forest", user: "34 197 94", assistant: "234 179 8" },
  { id: "sunset", label: "Sunset", user: "244 63 94", assistant: "251 146 60" },
  { id: "mono", label: "Monochrome", user: "148 163 184", assistant: "100 116 139" },
];

const STORAGE_KEY = "cairncc:palette";
const DEFAULT_ID: PaletteId = "indigo-orange";

function getInitial(): PaletteId {
  if (typeof window === "undefined") return DEFAULT_ID;
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved && PALETTE_PRESETS.some((p) => p.id === saved)) return saved as PaletteId;
  return DEFAULT_ID;
}

let _id = $state<PaletteId>(getInitial());

function applyToRoot(id: PaletteId) {
  if (typeof document === "undefined") return;
  const preset = PALETTE_PRESETS.find((p) => p.id === id) ?? PALETTE_PRESETS[0];
  const root = document.documentElement;
  root.style.setProperty("--bubble-user-rgb", preset.user);
  root.style.setProperty("--bubble-asst-rgb", preset.assistant);
}

export const paletteStore = {
  get id() {
    return _id;
  },
  set id(v: PaletteId) {
    _id = v;
    if (typeof window !== "undefined") {
      localStorage.setItem(STORAGE_KEY, v);
      applyToRoot(v);
    }
  },
  get current(): PalettePreset {
    return PALETTE_PRESETS.find((p) => p.id === _id) ?? PALETTE_PRESETS[0];
  },
  apply() {
    applyToRoot(_id);
  },
};
