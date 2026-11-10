'use strict';
// Local, permission-free display placement configuration.
// No synthetic monitor names or resolutions in browser preview.
let wfsDisplayInitialized = false;
let wfsDisplayRefreshing = false;

function wfsDisplayStatus(message, isError = false) {
  const node = document.querySelector('#display-status');
  if (!node) return;
  node.textContent = message;
  node.classList.toggle('error', isError);
}

function wfsDisplaySetControls(enabled) {
  for (const id of ['display-monitor', 'display-top-gap', 'display-apply', 'display-refresh']) {
    const element = document.getElementById(id);
    if (element) element.disabled = !enabled;
  }
}

async function wfsRefreshDisplays() {
  if (!nativeInvoke || wfsDisplayRefreshing) return;
  wfsDisplayRefreshing = true;
  const refreshButton = document.getElementById('display-refresh');
  refreshButton.disabled = true;
  try {
    const snapshot = await invoke('get_display_options');
    const monitors = Array.isArray(snapshot.displays) ? snapshot.displays : [];
    const select = document.getElementById('display-monitor');
    select.replaceChildren();
    const primary = document.createElement('option');
    primary.value = 'primary';
    primary.textContent = 'Primary display (automatic)';
    select.append(primary);
    for (const [index, display] of monitors.entries()) {
      const choice = document.createElement('option');
      choice.value = display.id;
      const scale = Number.isFinite(display.scale_factor) ? Math.round(display.scale_factor * 100) : '?';
      choice.textContent = (index + 1) + '. ' + display.name + ' · ' + display.width + '×' + display.height + ' · ' + scale + '%' + (display.is_primary ? ' · Primary' : '');
      select.append(choice);
    }
    const saved = snapshot.monitor_id || 'primary';
    if (saved !== 'primary' && !monitors.some(display => display.id === saved)) {
      const old = document.createElement('option');
      old.value = saved;
      old.textContent = snapshot.selected_monitor_unavailable
        ? 'Saved display disconnected — position retained, no automatic relocation'
        : 'Legacy coordinate preference — select this display again to adopt a stable ID';
      select.append(old);
    }
    select.value = saved;
    document.getElementById('display-top-gap').value =
      String(Number.isInteger(snapshot.margin_px) ? snapshot.margin_px : 12);
    const disconnected = Boolean(snapshot.selected_monitor_unavailable);
    const fallback = disconnected
      ? ' Saved display disconnected; WFS will not intentionally move the notch to another monitor. Always-on-top is disabled until it returns. Windows may independently move an existing window. Choose Primary if you want automatic placement.'
      : snapshot.using_fallback ? ' Primary monitor unavailable; using another connected display.' : '';
    const legacy = saved.startsWith('origin:') ? ' Coordinate-based saved selection; choose this monitor again to switch to a stable device name when available.' : '';
    wfsDisplayStatus(monitors.length + ' monitor(s) detected.' + fallback + legacy + ' Settings remain local on this computer.');
  } catch (error) {
    wfsDisplayStatus('Cannot detect displays: ' + (error?.message || String(error)), true);
  } finally {
    refreshButton.disabled = false;
    wfsDisplayRefreshing = false;
  }
}

async function wfsApplyDisplay() {
  if (!nativeInvoke) return;
  const monitorId = document.getElementById('display-monitor').value;
  const input = document.getElementById('display-top-gap');
  const marginPx = Number(input.value);
  if (!Number.isInteger(marginPx) || marginPx < 0 || marginPx > 120) {
    wfsDisplayStatus('Top gap must be a whole number from 0 to 120.', true);
    input.focus();
    return;
  }
  const apply = document.getElementById('display-apply');
  apply.disabled = true;
  try {
    if (!(await flushNote())) return;
    await writeQueue;
    const result = await invoke('set_display_preference', {monitorId, marginPx});
    data = result;
    render();
    await wfsRefreshDisplays();
    showInfo('Notch position saved.');
  } catch (error) {
    wfsDisplayStatus('Cannot move notch: ' + (error?.message || String(error)), true);
    showError(error);
  } finally {
    apply.disabled = false;
  }
}

function wfsInitDisplays() {
  if (wfsDisplayInitialized) return;
  wfsDisplayInitialized = true;
  document.querySelector('#display-refresh').addEventListener('click', () => void wfsRefreshDisplays());
  document.querySelector('#display-apply').addEventListener('click', () => void wfsApplyDisplay());
  document.querySelector('.tab[data-tab="settings"]').addEventListener('click', () => {
    if (nativeInvoke) setTimeout(() => { void wfsRefreshDisplays(); }, 0);
  });
  if (!nativeInvoke) {
    wfsDisplaySetControls(false);
    wfsDisplayStatus('Display selection requires the Windows desktop app. Browser preview cannot detect real monitors.');
    return;
  }
  void wfsRefreshDisplays();
}
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', wfsInitDisplays, {once:true});
} else {
  wfsInitDisplays();
}
