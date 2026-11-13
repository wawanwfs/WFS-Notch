'use strict';
// Phase 4 modules. Uses shared app.js state, mutation queue and UI helpers.
let extrasInitialized = false;
const swKey = 'wfs-notch-stopwatch-v1';
const stopwatch = { elapsed: 0, since: null, laps: [] };
let generatedPassword = '';

function localDay(d = new Date()) {
  const pad = n => String(n).padStart(2, '0');
  return [d.getFullYear(), pad(d.getMonth() + 1), pad(d.getDate())].join('-');
}
function dayStreak(dates) {
  const completed = new Set(dates);
  const day = new Date();
  day.setHours(12, 0, 0, 0);
  if (!completed.has(localDay(day))) day.setDate(day.getDate() - 1);
  let count = 0;
  while (completed.has(localDay(day)) && count < 3660) {
    count += 1;
    day.setDate(day.getDate() - 1);
  }
  return count;
}
function renderHabits() {
  const today = localDay();
  const count = data.habits.filter(h => h.completed_dates.includes(today)).length;
  $('#habit-count').textContent = count + ' OF ' + data.habits.length + ' TODAY';
  $('#home-habit-progress').textContent = count + ' / ' + data.habits.length;
  const list = $('#habit-list');
  list.replaceChildren();
  if (!data.habits.length) {
    list.append(node('div', 'empty', 'Your first daily habit starts here.'));
    return;
  }
  for (const habit of [...data.habits].reverse()) {
    const done = habit.completed_dates.includes(today);
    const line = node('div', 'module-item' + (done ? ' done' : ''));
    const toggle = node('button', 'habit-check', done ? '✓' : '○');
    toggle.type = 'button';
    toggle.setAttribute('aria-label', 'Complete today: ' + habit.title);
    toggle.setAttribute('aria-pressed', String(done));
    toggle.addEventListener('click', () => void mutation('toggle_habit_day', { id: habit.id, day: localDay() }));
    const info = node('div', 'module-info');
    info.append(node('b', '', habit.title));
    info.append(node('small', '', dayStreak(habit.completed_dates) + ' day streak · ' + habit.completed_dates.length + ' check-ins'));
    const remove = node('button', 'remove-task', '×');
    remove.type = 'button';
    remove.setAttribute('aria-label', 'Delete habit: ' + habit.title);
    remove.addEventListener('click', () => void mutation('remove_habit', { id: habit.id }));
    line.append(toggle, info, remove);
    list.append(line);
  }
}
function renderDock() {
  $('#home-link-count').textContent = String(data.quick_links.length);
  const list = $('#quicklink-list');
  list.replaceChildren();
  if (!data.quick_links.length) {
    list.append(node('div', 'empty', 'No websites pinned yet.'));
    return;
  }
  for (const link of data.quick_links) {
    const line = node('div', 'module-item');
    const open = node('button', 'link-open', link.label);
    open.type = 'button';
    open.title = link.url;
    open.setAttribute('aria-label', 'Open ' + link.label);
    open.addEventListener('click', async () => {
      try { await invoke('open_quick_link', { id: link.id }); }
      catch (error) { showError(error); }
    });
    const small = node('small', 'link-subtitle', link.url);
    const remove = node('button', 'remove-task', '×');
    remove.type = 'button';
    remove.setAttribute('aria-label', 'Unpin ' + link.label);
    remove.addEventListener('click', () => void mutation('remove_quick_link', { id: link.id }));
    const box = node('div', 'module-info'); box.append(open, small);
    line.append(box, remove); list.append(line);
  }
}
function renderExtras() {
  if (!$('#habit-list')) return;
  renderHabits();
  renderDock();
}
function stopwatchElapsed() {
  return stopwatch.elapsed + (stopwatch.since == null ? 0 : Math.max(0, Date.now() - stopwatch.since));
}
function renderStopwatch() {
  if (!$('#stopwatch-display')) return;
  const ms = stopwatchElapsed();
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor(ms / 1000) % 60;
  const tenths = Math.floor(ms / 100) % 10;
  $('#stopwatch-display').textContent = String(minutes).padStart(2, '0') + ':' + String(seconds).padStart(2, '0') + '.' + tenths;
  $('#stopwatch-toggle').textContent = stopwatch.since == null ? 'Start' : 'Pause';
  const laps = $('#stopwatch-laps');
  if (laps.dataset.count !== String(stopwatch.laps.length)) {
    laps.dataset.count = String(stopwatch.laps.length);
    laps.replaceChildren(...stopwatch.laps.slice(-5).reverse().map((ms, i) => node('small', '', 'Lap ' + (stopwatch.laps.length - i) + ' · ' + (ms / 1000).toFixed(1) + 's')));
  }
}
function saveStopwatch() {
  try { localStorage.setItem(swKey, JSON.stringify(stopwatch)); } catch (error) { console.warn('Stopwatch state not persisted', error); }
}
function randomPassword(length = 20) {
  if (!globalThis.crypto?.getRandomValues) throw new Error('Secure random generator unavailable');
  const alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%&*-_';
  const max = Math.floor(256 / alphabet.length) * alphabet.length;
  const out = [];
  while (out.length < length) {
    const bytes = crypto.getRandomValues(new Uint8Array(64));
    for (const byte of bytes) {
      if (byte < max) out.push(alphabet[byte % alphabet.length]);
      if (out.length === length) break;
    }
  }
  return out.join('');
}
function calculateSimple(expression) {
  const pattern = /^\s*(-?(?:\d+(?:\.\d+)?|\.\d+))\s*([+\-*/])\s*(-?(?:\d+(?:\.\d+)?|\.\d+))\s*$/;
  const m = expression.match(pattern);
  if (!m) throw new Error('Use one expression like 12.5 * 2');
  const left = Number(m[1]), right = Number(m[3]);
  const result = m[2] === '+' ? left + right : m[2] === '-' ? left - right : m[2] === '*' ? left * right : left / right;
  if (!Number.isFinite(result)) throw new Error('Invalid calculation (division by zero?)');
  return Number(result.toPrecision(12)).toString();
}
const moduleKeywords = {
  home: 'dashboard overview', todos: 'todo task reminders', notes: 'draft writing',
  calendar: 'events appointments ics', timer: 'pomodoro focus session',
  system: 'cpu ram disk network telemetry', health: 'water wellness break',
  day: 'schedule today', countdown: 'timer alarm', weather: 'temperature forecast',
  flashcards: 'study cards', habits: 'streak checklist', dock: 'bookmarks launcher links',
  media: 'music audio player volume', tools: 'stopwatch calculator password',
  settings: 'preferences appearance contrast density display backups',
};
let paletteOpener = null;
let paletteSession = 0;
let paletteSelecting = false;

function paletteIsOpen() { return !$('#command-palette').hidden; }

function setPaletteModality(open) {
  const overlay = $('#command-palette');
  for (const sibling of overlay.parentElement.children) {
    if (sibling !== overlay) sibling.inert = open;
  }
}

function openPalette() {
  if (!isExpanded || paletteIsOpen()) return;
  paletteSession += 1;
  paletteOpener = document.activeElement;
  $('#command-palette').hidden = false;
  setPaletteModality(true);
  $('#palette-search').value = '';
  renderPalette();
  $('#palette-search').focus();
}
function closePalette(focusTarget = paletteOpener) {
  if (!paletteIsOpen()) return;
  paletteSession += 1;
  if (paletteSelecting) tabRequest += 1;
  paletteSelecting = false;
  $('#command-palette').hidden = true;
  setPaletteModality(false);
  const target = focusTarget?.closest('#expanded') && focusTarget.getClientRects().length
    ? focusTarget : $('#module-finder');
  target.focus();
  paletteOpener = null;
}

async function selectPaletteModule(tab) {
  if (paletteSelecting) return;
  const session = paletteSession;
  paletteSelecting = true;
  const opened = await switchTab(tab.dataset.tab);
  if (session !== paletteSession) return;
  paletteSelecting = false;
  if (opened) closePalette(tab);
  else $('#palette-status').textContent = 'Module not opened. Close the finder to resolve the unsaved note.';
}

function renderPalette() {
  const terms = $('#palette-search').value.toLowerCase().trim().split(/\s+/).filter(Boolean);
  const container = $('#palette-results');
  container.replaceChildren();
  const tabs = $$('.tab').filter(tab => {
    const searchable = `${tab.textContent.toLowerCase()} ${moduleKeywords[tab.dataset.tab] || ''}`;
    return terms.every(term => searchable.includes(term));
  });
  $('#palette-status').textContent = tabs.length
    ? `${tabs.length} ${tabs.length === 1 ? 'module' : 'modules'} available`
    : 'No matching module. Try another search.';
  for (const tab of tabs) {
    const button = node('button', 'palette-option', tab.textContent);
    button.type = 'button';
    if (tab.dataset.tab === activeTab) {
      button.setAttribute('aria-current', 'page');
      button.append(node('span', 'palette-current', 'Current'));
    }
    button.addEventListener('click', () => void selectPaletteModule(tab));
    container.append(button);
  }
  container.scrollTop = 0;
}

function handlePaletteKey(event) {
  if (event.isComposing) return;
  const input = $('#palette-search');
  const options = $$('#palette-results .palette-option');
  const index = options.indexOf(document.activeElement);
  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    closePalette();
  } else if (event.key === 'Tab') {
    const stops = [$('#palette-close'), input, ...options];
    const current = stops.indexOf(document.activeElement);
    if ((event.shiftKey && current === 0) || (!event.shiftKey && current === stops.length - 1)) {
      event.preventDefault();
      stops[event.shiftKey ? stops.length - 1 : 0].focus();
    }
  } else if ((event.key === 'ArrowDown' || event.key === 'ArrowUp') &&
      (document.activeElement === input || index !== -1) && options.length) {
    event.preventDefault();
    const next = event.key === 'ArrowDown' ? index + 1 : index === -1 ? options.length - 1 : index - 1;
    (options[next] || input).focus();
  } else if (event.key === 'Enter' && document.activeElement === input) {
    event.preventDefault();
    options[0]?.click();
  }
}
function initExtras() {
  if (extrasInitialized) return;
  extrasInitialized = true;
  const stored = (() => {
    try { return JSON.parse(localStorage.getItem(swKey) || '{}'); } catch { return {}; }
  })();
  stopwatch.elapsed = Number.isFinite(stored.elapsed) && stored.elapsed >= 0 ? stored.elapsed : 0;
  stopwatch.since = Number.isFinite(stored.since) && stored.since > 0 && stored.since <= Date.now() ? stored.since : null;
  stopwatch.laps = Array.isArray(stored.laps) ? stored.laps.filter(v => Number.isFinite(v) && v >= 0).slice(-30) : [];

  $('#habit-form').addEventListener('submit', async event => {
    event.preventDefault();
    const input = $('#habit-title');
    const title = input.value.trim();
    if (title && await mutation('add_habit', { title })) input.value = '';
  });
  $('#quicklink-form').addEventListener('submit', async event => {
    event.preventDefault();
    const label = $('#quicklink-label').value.trim();
    const url = $('#quicklink-url').value.trim();
    if (label && url && await mutation('add_quick_link', { label, url })) {
      $('#quicklink-label').value = ''; $('#quicklink-url').value = '';
    }
  });
  $$('[data-launch]').forEach(button => button.addEventListener('click', async () => {
    try { await invoke('launch_builtin', { appName: button.dataset.launch }); }
    catch (error) { showError(error); }
  }));
  $$('[data-media]').forEach(button => button.addEventListener('click', async () => {
    try { await invoke('media_key', { action: button.dataset.media }); }
    catch (error) { showError(error); }
  }));
  const autostart = $('#autostart-toggle');
  if (!nativeInvoke) {
    autostart.disabled = true;
    autostart.title = 'Windows-only control';
  } else {
    invoke('get_autostart').then(value => { autostart.checked = value; })
      .catch(error => { autostart.disabled = true; showError(error); });
  }
  autostart.addEventListener('change', async () => {
    const requested = autostart.checked;
    autostart.disabled = true;
    try { autostart.checked = await invoke('set_autostart', { enabled: requested }); }
    catch (error) { autostart.checked = !requested; showError(error); }
    finally { autostart.disabled = false; }
  });
  $('#stopwatch-toggle').addEventListener('click', () => {
    if (stopwatch.since == null) stopwatch.since = Date.now();
    else { stopwatch.elapsed = stopwatchElapsed(); stopwatch.since = null; }
    saveStopwatch(); renderStopwatch();
  });
  $('#stopwatch-reset').addEventListener('click', () => {
    stopwatch.elapsed = 0; stopwatch.since = null; stopwatch.laps = [];
    $('#stopwatch-laps').dataset.count = '';
    saveStopwatch(); renderStopwatch();
  });
  $('#stopwatch-lap').addEventListener('click', () => {
    if (stopwatch.since == null) return;
    stopwatch.laps.push(stopwatchElapsed());
    stopwatch.laps = stopwatch.laps.slice(-30);
    saveStopwatch(); renderStopwatch();
  });
  $('#password-generate').addEventListener('click', () => {
    try { generatedPassword = randomPassword(); $('#password-output').textContent = generatedPassword; }
    catch (error) { showError(error); }
  });
  $('#password-copy').addEventListener('click', async () => {
    if (!generatedPassword) { showInfo('Generate a password first.'); return; }
    try { await navigator.clipboard.writeText(generatedPassword); showInfo('Copied to clipboard.'); }
    catch { showError('Clipboard permission unavailable. Select the generated password manually.'); }
  });
  $('#math-form').addEventListener('submit', event => {
    event.preventDefault();
    try { $('#math-output').textContent = calculateSimple($('#math-input').value); }
    catch (error) { showError(error); }
  });
  $('#palette-open').addEventListener('click', openPalette);
  $('#module-finder').addEventListener('click', openPalette);
  $('#palette-close').addEventListener('click', () => closePalette());
  $('#palette-search').addEventListener('input', renderPalette);
  $('#command-palette').addEventListener('keydown', handlePaletteKey);
  $('#command-palette').addEventListener('click', event => {
    if (event.target === event.currentTarget) closePalette();
  });
  document.addEventListener('keydown', async event => {
    if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey || event.repeat ||
        event.isComposing || event.key.toLowerCase() !== 'k') return;
    event.preventDefault();
    if (!isExpanded) await setExpanded(true);
    openPalette();
  });
  setInterval(renderStopwatch, 100);
  renderStopwatch();
  renderExtras();
}
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', initExtras, { once: true });
else initExtras();
