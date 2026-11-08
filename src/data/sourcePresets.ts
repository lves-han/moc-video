import defaults from "./defaultSources.json";

// Keep the source manager and first-run configuration on the same preset list.
export const SOURCE_PRESETS = defaults.filter(source => source.kind === "maccms")
  .map(source => ({ name: source.name, api_url: source.api_url! }));
