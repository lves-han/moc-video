import defaults from "./defaultSources.json" with { type: "json" };
import type { HttpSourceConfig } from "../models";

// Bundled MacCMS presets; first-run settings use the same list.
export const SOURCE_PRESETS = defaults.filter(source => source.kind === "maccms")
  .map(source => ({ name: source.name, api_url: source.api_url! }));

// Presets not yet added, shown after configured sources in the one playback source list.
export const unaddedPresets = (configs: HttpSourceConfig[]) =>
  SOURCE_PRESETS.filter(preset => !configs.some(config => config.kind === "maccms" && config.api_url === preset.api_url));
