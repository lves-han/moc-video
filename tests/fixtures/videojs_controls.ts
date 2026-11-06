// Tests query the shipped skin's open shadow root, never a duplicate application control bar.
export function videoJsControls() {
  const skin = document.querySelector('video-scaffold-skin');
  if (!skin?.shadowRoot) throw new Error('Official Video.js skin is not mounted');
  return skin.shadowRoot;
}
export function videoJsControl(name: 'play' | 'mute' | 'fullscreen' | 'seek' | 'volume') {
  const tags = { play: 'media-play-button', mute: 'media-mute-button', fullscreen: 'media-fullscreen-button', seek: 'media-time-slider', volume: 'media-volume-slider' };
  const control = videoJsControls().querySelector<HTMLElement>(tags[name]);
  if (!control) throw new Error(`Official Video.js ${name} control missing`);
  return control;
}
