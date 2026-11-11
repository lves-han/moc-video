// Query the composed skin's real official elements. The application has one control surface.
export function videoJsControls() {
  const player = document.querySelector('video-player');
  if (!player?.querySelector('media-container.playback-skin') || player.querySelectorAll('media-controls').length !== 1
      || document.querySelector('video-compat-skin')) throw new Error('Composed official Video.js skin is not mounted exactly once');
  return document;
}
export function videoJsControl(name: 'play' | 'mute' | 'fullscreen' | 'seek' | 'volume') {
  const tags = { play: 'media-play-button', mute: 'media-mute-button', fullscreen: 'media-fullscreen-button', seek: 'media-time-slider', volume: 'media-volume-slider' };
  const control = videoJsControls().querySelector<HTMLElement>(tags[name]);
  if (!control) throw new Error(`Official Video.js ${name} control missing`);
  return control;
}
