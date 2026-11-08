'use strict';
// Privacy-preserving Windows GSMTC snapshot. Never create fake music metadata for browser preview.
let mediaDetailsInitialized = false;
let mediaDetailsPending = false;
let mediaDetailsLastAt = 0;
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
  if (!snapshot) {
    track.textContent = 'No active media session';
    artist.textContent = nativeInvoke ? 'Start music in a supported Windows player.' : 'Track metadata requires the installed Windows desktop application.';
    source.textContent = nativeInvoke ? 'Windows global media session' : 'Browser preview — no synthetic media';
    pos.textContent = '—'; duration.textContent = '—'; status.textContent = 'Unavailable';
    bar.style.width = '0%';
    return;
  }
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
    renderMediaDetails(await invoke('get_active_media_session'));
    mediaDetailsLastAt = Date.now();
  } catch (error) {
    renderMediaDetails(null);
    document.querySelector('#media-track-artist').textContent = 'Windows media sessions are unavailable: ' + (error?.message || String(error));
  } finally {
    mediaDetailsPending = false;
  }
}
function initMediaSessions() {
  if (mediaDetailsInitialized) return;
  mediaDetailsInitialized = true;
  renderMediaDetails(null);
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
}
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', initMediaSessions, {once:true});
else initMediaSessions();
