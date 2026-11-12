import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import {
  paletteOptions,
  resolveTheme,
  normalizePalette,
} from "../src/ui/theme.ts";

const html = readFileSync(new URL("../index.html", import.meta.url), "utf8");
const bootstrap = html.match(/<script>([\s\S]*?)<\/script>/)![1];
const palettes = ["lime", "ocean", "violet", "rose", "amber"];

test("Startup restores every supported palette and ignores old manual light/dark preferences", () => {
  for (const saved of [...palettes, "unknown", null]) {
    const dataset = { palette: "lime" };
    runInNewContext(bootstrap, {
      document: { documentElement: { dataset } },
      localStorage: {
        getItem(key: string) {
          return key === "moc-video.palette" ? saved : "dark";
        },
      },
    });
    assert.equal(dataset.palette, palettes.includes(saved!) ? saved : "lime");
    assert.equal("theme" in dataset, false);
  }
  const dataset = { palette: "lime" };
  runInNewContext(bootstrap, {
    document: { documentElement: { dataset } },
    localStorage: {
      getItem() {
        throw new Error("Storage unavailable");
      },
    },
  });
  assert.equal(dataset.palette, "lime");
});

function luminance(hex: string) {
  const channel = [1, 3, 5]
    .map((i) => parseInt(hex.slice(i, i + 2), 16) / 255)
    .map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return channel[0] * 0.2126 + channel[1] * 0.7152 + channel[2] * 0.0722;
}
function contrast(first: string, second: string) {
  const [low, high] = [luminance(first), luminance(second)].sort(
    (a, b) => a - b,
  );
  return (high + 0.05) / (low + 0.05);
}

test("All palette presets share their accessible colors between Naive UI and the cinema layouts", () => {
  for (const item of paletteOptions) {
    const day = resolveTheme(item.value, false);
    const night = resolveTheme(item.value, true);
    assert.equal(day.overrides.common?.primaryColor, day.variables["--accent"]);
    assert.equal(
      night.overrides.common?.primaryColor,
      night.variables["--accent"],
    );
    assert.equal(day.theme, null);
    assert.ok(night.theme);
    for (const [first, second] of [
      [day.variables["--accent"], "#f7f8fa"],
      [day.variables["--accent-ink"], day.variables["--accent"]],
      [day.variables["--accent"], day.variables["--accent-soft"]],
      [night.variables["--accent"], "#101014"],
      [night.variables["--accent"], night.variables["--accent-soft"]],
      [night.variables["--accent-ink"], night.variables["--accent"]],
    ])
      assert.ok(
        contrast(first, second) >= 4.5,
        `${item.value}: ${first} / ${second}`,
      );
  }
  assert.equal(normalizePalette("unknown"), "lime");
  assert.equal(normalizePalette(null), "lime");
});

test("TV1 desktop remains dark with every night accent under both system appearances", () => {
  for (const brand of paletteOptions) for (const systemDark of [false, true]) {
    const result = resolveTheme(brand.value, systemDark, true);
    assert.ok(result.theme);
    assert.equal(result.variables["--accent"], brand.night);
    assert.equal(result.variables["--background"], "#0b0b0d");
    assert.equal(result.variables["--tv-text"], "#f5f5f7");
    assert.equal(result.overrides.common?.bodyColor, result.variables["--tv-bg"]);
    assert.equal(result.overrides.common?.textColor1, result.variables["--tv-text"]);
  }
  assert.equal("--tv-bg" in resolveTheme("lime", false).variables, false);
  assert.equal(resolveTheme("lime", false).theme, null);
});
