import {
  loadThemes,
  saveThemes,
  type Theme as CustomTheme,
  type ThemeData,
} from "$lib/themes/bridge";

type BaseTheme = "dark" | "light";

export const THEME_COLOR_KEYS = [
  "bg-primary",
  "bg-secondary",
  "bg-tertiary",
  "bg-hover",
  "text-primary",
  "text-secondary",
  "text-muted",
  "accent",
  "accent-hover",
  "accent-link",
  "accent-code",
  "danger",
  "danger-hover",
  "success",
  "warning",
  "border",
  "border-subtle",
] as const;

export type ThemeColorKey = (typeof THEME_COLOR_KEYS)[number];

export const THEME_COLOR_LABELS: Record<ThemeColorKey, string> = {
  "bg-primary": "Background",
  "bg-secondary": "Surface",
  "bg-tertiary": "Surface Alt",
  "bg-hover": "Hover",
  "text-primary": "Text",
  "text-secondary": "Text Secondary",
  "text-muted": "Text Muted",
  accent: "Accent",
  "accent-hover": "Accent Hover",
  "accent-link": "Links",
  "accent-code": "Inline Code",
  danger: "Danger",
  "danger-hover": "Danger Hover",
  success: "Success",
  warning: "Warning",
  border: "Border",
  "border-subtle": "Border Subtle",
};

export const DARK_DEFAULTS: Record<ThemeColorKey, string> = {
  "bg-primary": "#0a0a0a",
  "bg-secondary": "#111111",
  "bg-tertiary": "#191919",
  "bg-hover": "#1e1e1e",
  "text-primary": "#dcddde",
  "text-secondary": "#999999",
  "text-muted": "#666666",
  accent: "#dcddde",
  "accent-hover": "#ffffff",
  "accent-link": "#ff6633",
  "accent-code": "#e06c75",
  danger: "#ef4444",
  "danger-hover": "#f87171",
  success: "#22c55e",
  warning: "#eab308",
  border: "#262626",
  "border-subtle": "#1a1a1a",
};

export const LIGHT_DEFAULTS: Record<ThemeColorKey, string> = {
  "bg-primary": "#fafafa",
  "bg-secondary": "#f5f5f5",
  "bg-tertiary": "#e5e5e5",
  "bg-hover": "#d4d4d4",
  "text-primary": "#2e3338",
  "text-secondary": "#666666",
  "text-muted": "#aaaaaa",
  accent: "#2e3338",
  "accent-hover": "#000000",
  "accent-link": "#dc501e",
  "accent-code": "#c0392b",
  danger: "#ef4444",
  "danger-hover": "#dc2626",
  success: "#16a34a",
  warning: "#ca8a04",
  border: "#e5e5e5",
  "border-subtle": "#f0f0f0",
};

let baseTheme = $state<BaseTheme>(
  (typeof localStorage !== "undefined" &&
    (localStorage.getItem("margin-theme") as BaseTheme)) ||
    "dark",
);

let customThemes = $state<CustomTheme[]>([]);
let activeThemeName = $state<string | null>(
  (typeof localStorage !== "undefined" &&
    localStorage.getItem("margin-active-theme")) ||
    null,
);
let themesLoaded = $state(false);

function applyBaseTheme(t: BaseTheme) {
  if (typeof document !== "undefined") {
    document.documentElement.setAttribute("data-theme", t);
  }
}

function applyCustomColors(colors: Record<string, string>) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  for (const key of THEME_COLOR_KEYS) {
    root.style.removeProperty(`--${key}`);
  }
  for (const [key, value] of Object.entries(colors)) {
    if (value) root.style.setProperty(`--${key}`, value);
  }
}

function clearCustomColors() {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  for (const key of THEME_COLOR_KEYS) {
    root.style.removeProperty(`--${key}`);
  }
}

$effect.root(() => {
  applyBaseTheme(baseTheme);

  loadThemes()
    .then((data) => {
      customThemes = data.themes;
      if (data.active_theme) {
        activeThemeName = data.active_theme;
      }
      const active = customThemes.find((t) => t.name === activeThemeName);
      if (active) {
        applyCustomColors(active.colors);
      }
      themesLoaded = true;
    })
    .catch(() => {
      themesLoaded = true;
    });
});

async function persistThemes() {
  const data: ThemeData = {
    themes: customThemes,
    active_theme: activeThemeName,
  };
  await saveThemes(data);
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("margin-themes-backup", JSON.stringify(data));
    localStorage.setItem("margin-active-theme", activeThemeName ?? "");
  }
}

export const theme = {
  get current() {
    return baseTheme;
  },

  get customThemes() {
    return customThemes;
  },

  get activeThemeName() {
    return activeThemeName;
  },

  get activeTheme(): CustomTheme | null {
    if (!activeThemeName) return null;
    return customThemes.find((t) => t.name === activeThemeName) ?? null;
  },

  get loaded() {
    return themesLoaded;
  },

  toggle() {
    baseTheme = baseTheme === "dark" ? "light" : "dark";
    applyBaseTheme(baseTheme);
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("margin-theme", baseTheme);
    }
    const active = customThemes.find((t) => t.name === activeThemeName);
    if (active) {
      applyCustomColors(active.colors);
    }
  },

  async addTheme(t: CustomTheme) {
    const idx = customThemes.findIndex((x) => x.name === t.name);
    if (idx >= 0) {
      customThemes[idx] = t;
    } else {
      customThemes.push(t);
    }
    customThemes = [...customThemes];
    await persistThemes();
  },

  async deleteTheme(name: string) {
    customThemes = customThemes.filter((t) => t.name !== name);
    if (activeThemeName === name) {
      activeThemeName = null;
      clearCustomColors();
    }
    await persistThemes();
  },

  async activateTheme(name: string | null) {
    activeThemeName = name;
    if (name) {
      const t = customThemes.find((x) => x.name === name);
      if (t) applyCustomColors(t.colors);
    } else {
      clearCustomColors();
    }
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("margin-active-theme", name ?? "");
    }
    await persistThemes();
  },

  previewColors(colors: Record<string, string>) {
    applyCustomColors(colors);
  },

  cancelPreview() {
    const active = customThemes.find((t) => t.name === activeThemeName);
    if (active) {
      applyCustomColors(active.colors);
    } else {
      clearCustomColors();
    }
  },
};
