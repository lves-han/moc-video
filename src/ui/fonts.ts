import catalog from '../data/fontCatalog.json' with { type: 'json' };

export const fontOptions = catalog;
export function setFontScale(value: number = 100) {
  const scale = [90, 100, 110, 120].includes(value) ? value : 100;
  document.documentElement.style.zoom = String(scale / 100);
}
export function setAppFont(id: unknown) {
  const font = fontOptions.find(item => item.id === id) ?? fontOptions[0]!;
  document.documentElement.style.setProperty('--app-font', `"${font.family}", system-ui, sans-serif`);
}
