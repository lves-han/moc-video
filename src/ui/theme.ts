import { ref } from "vue";
import { darkTheme, lightTheme, type GlobalThemeOverrides } from "naive-ui";

// Brand colors are the only overrides to Naive UI's existing light/dark themes.
export const paletteOptions = [
  {
    value: "lime",
    label: "Lime",
    day: "#466827",
    night: "#c5ed88",
    ink: "#243218",
    hoverDay: "#385420",
    hoverNight: "#d7f6ad",
    softDay: "#eaf1e2",
    softNight: "#293225",
    lineDay: "#93aa7e",
    lineNight: "#71865a",
  },
  {
    value: "ocean",
    label: "Ocean",
    day: "#225c9d",
    night: "#a7d0ff",
    ink: "#142c49",
    hoverDay: "#19497d",
    hoverNight: "#c8e2ff",
    softDay: "#e7f0fb",
    softNight: "#202f42",
    lineDay: "#86a5c8",
    lineNight: "#607fa2",
  },
  {
    value: "violet",
    label: "Violet",
    day: "#6d42a4",
    night: "#cdb8ff",
    ink: "#30204a",
    hoverDay: "#573483",
    hoverNight: "#e1d3ff",
    softDay: "#f0eafa",
    softNight: "#30263e",
    lineDay: "#b09bc7",
    lineNight: "#8972a3",
  },
  {
    value: "rose",
    label: "Rose",
    day: "#a03662",
    night: "#f5b6ce",
    ink: "#481d30",
    hoverDay: "#82294e",
    hoverNight: "#ffd2e2",
    softDay: "#fbe9f0",
    softNight: "#3c2631",
    lineDay: "#c793a8",
    lineNight: "#a57286",
  },
  {
    value: "amber",
    label: "Amber",
    day: "#8c5b0a",
    night: "#f0ca85",
    ink: "#3e2b10",
    hoverDay: "#714807",
    hoverNight: "#ffdfab",
    softDay: "#fbf0dc",
    softNight: "#372f23",
    lineDay: "#b99a63",
    lineNight: "#9f8557",
  },
] as const;
export type PaletteId = (typeof paletteOptions)[number]["value"];
export function normalizePalette(value: unknown): PaletteId {
  return paletteOptions.find((item) => item.value === value)?.value || "lime";
}
export const palette = ref<PaletteId>(
  normalizePalette(
    typeof document === "undefined"
      ? null
      : document.documentElement.dataset.palette,
  ),
);
export function setPalette(value: PaletteId): boolean {
  palette.value = value;
  document.documentElement.dataset.palette = value;
  try {
    localStorage.setItem("yingxiang-video.palette", value);
    return true;
  } catch {
    return false;
  }
}

export function resolveTheme(id: PaletteId, dark: boolean, desktop = false) {
  dark = desktop || dark;
  const brand = paletteOptions.find((item) => item.value === id)!;
  const base = dark ? darkTheme : lightTheme;
  const primary = dark ? brand.night : brand.day;
  const hover = dark ? brand.hoverNight : brand.hoverDay;
  const ink = dark ? brand.ink : "#ffffff";
  const overrides: GlobalThemeOverrides = {
    common: {
      primaryColor: primary,
      primaryColorHover: hover,
      primaryColorPressed: hover,
      primaryColorSuppl: primary,
      borderRadius: "8px",
      heightMedium: "40px",
      heightSmall: "34px",
      bodyColor: desktop ? "#0b0b0d" : dark ? base.common.bodyColor : "#f7f8fa",
      ...(desktop ? {
        textColor1: "#f5f5f7", textColor2: "rgba(235,235,245,.62)", textColor3: "rgba(235,235,245,.38)",
        cardColor: "rgba(255,255,255,.06)", inputColor: "rgba(255,255,255,.10)",
        hoverColor: "rgba(255,255,255,.16)", borderColor: "rgba(255,255,255,.10)",
        popoverColor: "#1e1e22", errorColor: "#ff453a", fontSize: "15px", borderRadius: "20px",
      } : {}),
      placeholderColor: dark ? "#a3a8b0" : "#65707c",
      fontFamily:
        '-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif',
    },
    Button: {
      textColorPrimary: ink,
      textColorHoverPrimary: ink,
      textColorPressedPrimary: ink,
      textColorFocusPrimary: ink,
    },
    Switch: { buttonColor: ink },
  };
  const common = { ...base.common, ...overrides.common };
  return {
    theme: dark ? darkTheme : null,
    overrides,
    // Page-specific cinema layouts consume the same semantic values as the library.
    variables: {
      ...(desktop ? {
        "--tv-bg": "#0b0b0d", "--tv-surface-1": "rgba(255,255,255,.06)",
        "--tv-surface-2": "rgba(255,255,255,.10)", "--tv-surface-hover": "rgba(255,255,255,.16)",
        "--tv-glass": "rgba(30,30,34,.62)", "--tv-glass-filter": "blur(30px) saturate(160%)",
        "--tv-hairline": "rgba(255,255,255,.10)", "--tv-text": "#f5f5f7",
        "--tv-text-2": "rgba(235,235,245,.62)", "--tv-text-3": "rgba(235,235,245,.38)",
        "--tv-button": "#f5f5f7", "--tv-button-ink": "#0b0b0d", "--tv-danger": "#ff453a",
        "--tv-ease": "cubic-bezier(.2,.8,.2,1)", "--tv-focus-duration": "200ms",
        "--tv-page-duration": "300ms", "--tv-hero-duration": "600ms", "--tv-focus-scale": "1.08",
      } : {}),
      "--text": common.textColor1,
      "--background": common.bodyColor,
      "--surface": common.cardColor,
      "--surface-hover": common.hoverColor,
      "--popover": common.popoverColor,
      "--border": common.borderColor,
      "--control-surface": common.inputColor,
      "--control-border": common.borderColor,
      "--muted": common.textColor2,
      "--subtle": common.textColor2,
      "--accent": primary,
      "--accent-ink": ink,
      "--accent-hover": hover,
      "--accent-soft": dark ? brand.softNight : brand.softDay,
      "--accent-border": dark ? brand.lineNight : brand.lineDay,
      "--accent-night": brand.night,
      "--ink-night": brand.ink,
      "--hover-night": brand.hoverNight,
      "--shadow": common.boxShadow2,
    },
  };
}
