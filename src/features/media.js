'use strict';
// Privacy-preserving Windows GSMTC snapshot. Never create fake music metadata for browser preview.
let mediaDetailsInitialized = false;
let mediaDetailsPending = false;
let mediaDetailsLastAt = 0;
let mediaLastSample = null;
let mediaLastSampleAt = 0;
let mediaSessionChoices = [];
function mediaTime(ms) {
  if (!Number.isFinite(ms) || ms < 0) return '—';
  const secs = Math.floor(ms / 1000);
  return String(Math.floor(secs / 60)).padStart(2, '0') + ':' + String(secs % 60).padStart(2, '0');
}
function renderMediaDetails(snapshot) {
  const track = document.querySelector('#media-track-title');
  const artist = document.querySelector('#media-track-artist');
  const source = document.querySelector('#media-track-source');
  const pos = document.querySelector('#media-position');
  const duration = document.querySelector('#media-duration');
  const status = document.querySelector('#media-playback-state');
  const bar = document.querySelector('#media-progress-bar');
  const artwork = document.querySelector('#media-artwork');
  const card = document.querySelector('#now-playing');
  mediaLastSample = snapshot;
  mediaLastSampleAt = Date.now();
  if (!snapshot) {
    track.textContent = 'No active media session';
    artist.textContent = nativeInvoke ? 'Start music in a supported Windows player.' : 'Track metadata requires the installed Windows desktop application.';
    source.textContent = nativeInvoke ? 'Windows global media session' : 'Browser preview — no synthetic media';
    pos.textContent = '—'; duration.textContent = '—'; status.textContent = 'Unavailable';
    bar.style.width = '0%';
    artwork.hidden = true;
    artwork.removeAttribute('src');
    card.classList.remove('has-art');
    return;
  }
  const url = snapshot.artwork_data_url;
  const validArt = typeof url === 'string' && url.length <= 750000 && /^data:image\/(png|jpeg|gif);base64,[a-zA-Z0-9+/=]+$/.test(url);
  artwork.hidden = !validArt;
  if (validArt) artwork.src = url;
  else artwork.removeAttribute('src');
  card.classList.toggle('has-art', validArt);
  track.textContent = snapshot.title || 'Untitled track';
  artist.textContent = snapshot.artist || snapshot.album || 'Unknown artist';
  source.textContent = [snapshot.album, snapshot.source].filter(Boolean).join(' · ').slice(0, 250);
  pos.textContent = mediaTime(snapshot.position_ms);
  duration.textContent = mediaTime(snapshot.duration_ms);
  status.textContent = snapshot.playback_status || 'Unknown';
  const p = Number(snapshot.position_ms), d = Number(snapshot.duration_ms);
  bar.style.width = Number.isFinite(p) && Number.isFinite(d) && d > 0
    ? Math.min(100, Math.max(0, 100 * p / d)).toFixed(1) + '%' : '0%';
}
async function refreshMediaDetails(force = false) {
  if (!nativeInvoke || mediaDetailsPending) return;
  const panel = document.querySelector('#panel-media');
  if (!panel?.classList.contains('active')) return;
  if (!force && Date.now() - mediaDetailsLastAt < 5000) return;
  mediaDetailsPending = true;
  try {
    const active = await invoke('get_active_media_session');
    const sessions = await invoke('list_media_sessions');
    renderSelectedPlayers(sessions, active?.source);
    const picker = document.querySelector('#media-player-picker');
    const selected = picker.value;
    const choice = mediaSessionChoices.find(item => item.source === selected);
    if (choice?.ambiguous) {
      renderMediaDetails(null);
      document.querySelector('#media-player-status').textContent =
        'Multiple tabs or players share this Windows app ID. Individual session selection is unavailable; playback controls are disabled to avoid acting on the wrong tab.';
      mediaDetailsLastAt = Date.now();
      return;
    }
    const snapshot = selected && selected !== active?.source
      ? await invoke('get_media_session_by_source', { source: selected }) : active;
    if (!panel.classList.contains('active') || picker.value !== selected) return;
    renderMediaDetails(snapshot);
    if (selected && !snapshot) {
      document.querySelector('#media-player-status').textContent =
        'Selected session disconnected. Refresh or choose another player.';
    }
    mediaDetailsLastAt = Date.now();
  } catch (error) {
    renderMediaDetails(null);
    document.querySelector('#media-track-artist').textContent = 'Windows media sessions are unavailable: ' + (error?.message || String(error));
  } finally {
    mediaDetailsPending = false;
  }
}
function updateInterpolatedProgress() {
  const panel = document.querySelector('#panel-media');
  const snapshot = mediaLastSample;
  if (!panel?.classList.contains('active') || !snapshot || snapshot.playback_status !== 'Playing') return;
  const duration = Number(snapshot.duration_ms);
  const start = Number(snapshot.position_ms);
  if (!Number.isFinite(duration) || duration <= 0 || !Number.isFinite(start)) return;
  const age = Date.now() - mediaLastSampleAt;
  // A paused/suspended WebView may resume long after the previous reading.
  // Never project beyond a short bounded sample interval: wait for a fresh
  // GSMTC snapshot instead of showing a fabricated progress value.
  if (age < 0 || age > 10_000) return;
  const position = Math.min(duration, start + age);
  document.querySelector('#media-position').textContent = mediaTime(position);
  document.querySelector('#media-progress-bar').style.width =
    Math.max(0, Math.min(100, 100 * position / duration)).toFixed(1) + '%';
}

function renderSelectedPlayers(sessions, systemCurrentSource) {
  const picker = document.querySelector('#media-player-picker');
  const status = document.querySelector('#media-player-status');
  const safe = Array.isArray(sessions) ? sessions.filter(item => typeof item.source === 'string' && item.source.length > 0) : [];
  const previous = picker.value;
  mediaSessionChoices = safe;
  picker.replaceChildren();
  if (!safe.length) {
    const empty = document.createElement('option');
    empty.value = '';
    empty.textContent = 'No controllable media players';
    picker.append(empty);
    picker.disabled = true;
    status.textContent = 'No Windows GSMTC sessions available right now.';
  } else {
    for (const session of safe) {
      const option = document.createElement('option');
      option.value = session.source;
      option.textContent = ((session.title || session.source) + ' · ' + (session.artist || session.playback_status || 'Unknown') +
        (session.ambiguous === true ? ' · Multiple sessions: controls unavailable' : '')).slice(0, 190);
      picker.append(option);
    }
    const unambiguous = safe.filter(item => item.ambiguous !== true);
    const preferred = safe.some(item => item.source === previous) ? previous
      : unambiguous.some(item => item.source === systemCurrentSource) ? systemCurrentSource
      : unambiguous[0]?.source;
    picker.value = preferred || safe[0].source;
    picker.disabled = false;
    status.textContent = unambiguous.length
      ? 'Controls target only a uniquely identified Windows media app; options with multiple tabs are deliberately blocked.'
      : 'Multiple sessions share their app identity; controls are disabled to avoid changing the wrong tab.';
  }
  const chosen = safe.find(item => item.source === picker.value);
  for (const button of document.querySelectorAll('[data-selected-media]')) {
    const action = button.dataset.selectedMedia;
    const flag = action === 'toggle' ? 'can_toggle'
      : action === 'next' ? 'can_next' : 'can_previous';
    button.disabled = picker.disabled || !chosen || chosen.ambiguous === true || chosen[flag] !== true;
  }
}
async function controlSelectedPlayer(action) {
  const picker = document.querySelector('#media-player-picker');
  const status = document.querySelector('#media-player-status');
  if (!nativeInvoke || !picker.value) return;
  const source = picker.value;
  const selected = mediaSessionChoices.find(item => item.source === source);
  if (!selected || selected.ambiguous === true) {
    status.textContent = 'Control refused: choose a uniquely identifiable Windows media session.';
    return;
  }
  for (const button of document.querySelectorAll('[data-selected-media]')) button.disabled = true;
  try {
    const accepted = await invoke('control_media_session', {source, action});
    if (picker.value === source) status.textContent = accepted
      ? 'Command accepted by the selected media player.'
      : 'The selected player did not accept that command.';
    await refreshMediaDetails(true);
  } catch (error) {
    status.textContent = 'Selected media session unavailable. Refresh the list and retry.';
    showError(error);
  } finally {
    // Do not re-enable controls unsupported by the current player.
    const snapshot = mediaSessionChoices.find(item => item.source === picker.value);
    for (const button of document.querySelectorAll('[data-selected-media]')) {
      const flag = button.dataset.selectedMedia === 'toggle' ? 'can_toggle'
        : button.dataset.selectedMedia === 'next' ? 'can_next' : 'can_previous';
      button.disabled = picker.disabled || !snapshot || snapshot.ambiguous === true || snapshot[flag] !== true;
    }
  }
}
function initMediaSessions() {
  if (mediaDetailsInitialized) return;
  mediaDetailsInitialized = true;
  renderMediaDetails(null);
  for (const button of document.querySelectorAll('[data-selected-media]')) {
    button.addEventListener('click', () => { void controlSelectedPlayer(button.dataset.selectedMedia); });
    button.disabled = !nativeInvoke;
  }
  document.querySelector('#media-player-picker').addEventListener('change', () => {
    void refreshMediaDetails(true);
  });
  document.querySelector('#media-refresh').addEventListener('click', () => {
    void refreshMediaDetails(true);
  });
  document.querySelector('[data-tab="media"]').addEventListener('click', () => {
    setTimeout(() => { void refreshMediaDetails(); }, 0);
  });
  for (const button of document.querySelectorAll('[data-media]')) {
    button.addEventListener('click', () => {
      setTimeout(() => { void refreshMediaDetails(true); }, 700);
    });
  }
  setInterval(() => { void refreshMediaDetails(); }, 2500);
  setInterval(updateInterpolatedProgress, 500);
}
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', initMediaSessions, {once:true});
else initMediaSessions();
