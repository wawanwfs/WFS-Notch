'use strict';

const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => [...document.querySelectorAll(selector)];
const nativeInvoke = window.__TAURI__?.core?.invoke;
const previewKey = 'wfs-notch-browser-preview-v2';
const previewBackupsKey = 'wfs-notch-browser-backups-v3';
const defaults = () => ({
  todos: [], notes: [], events: [], habits: [], quick_links: [],
  health: { day: '', water_glasses: 0, break_count: 0, calm_minutes: 0 },
  countdown: { duration_seconds: 300, remaining_seconds: 300, ends_at_ms: null, completed: false },
  settings: { accent: '#d6b77a', hover_to_open: true, pin_open: false, monitor_id: 'primary', top_margin_px: 12, notifications_enabled: false, notifications_show_details: false, notifications_paused: false, always_on_top: true, auto_lower_fullscreen: false },
  focus: { duration_seconds: 1500, remaining_seconds: 1500, ends_at_ms: null, completed: false, paused: false },
});
let data = defaults();
let selectedNoteId = null;
let selectedEventId = null;
let selectedSeriesEventId = null;
let boundEditorId = null;
let pendingDraft = null;
let noteSaveTimeout = null;
let noteSaveInFlight = null;
let toastTimeout = null;
let activeTab = 'home';
let backupList = [];
let reminderSeen = new Set();
let focusCompletionNotified = false;
let telemetryPending = false;
let telemetryLastAt = 0;
let isExpanded = false;
let windowQueue = Promise.resolve();
let writeQueue = Promise.resolve();

function focusView(focus, current = Date.now()) {
  const remaining = focus.ends_at_ms == null
    ? focus.remaining_seconds
    : Math.max(0, Math.ceil((focus.ends_at_ms - current) / 1000));
  return {
    remaining,
    running: focus.ends_at_ms != null && remaining > 0,
    completed: (focus.ends_at_ms != null && remaining === 0) || (focus.completed && remaining === 0),
  };
}

function previewInvoke(command, args = {}) {
  let state;
  try {
    const raw = localStorage.getItem(previewKey) || localStorage.getItem('wfs-notch-browser-preview-v1');
    state = raw ? JSON.parse(raw) : defaults();
  } catch (error) { throw new Error(`Browser preview data is unreadable: ${error.message}`); }
  state.settings = { ...defaults().settings, ...state.settings };
  state.focus = { ...defaults().focus, ...state.focus };
  state.events = Array.isArray(state.events) ? state.events : [];
  state.habits = Array.isArray(state.habits) ? state.habits : [];
  state.quick_links = Array.isArray(state.quick_links) ? state.quick_links : [];
  state.health = { ...defaults().health, ...state.health };
  state.countdown = { ...defaults().countdown, ...state.countdown };
  if (command === 'list_backups') return Promise.resolve(JSON.parse(localStorage.getItem(previewBackupsKey) || '[]').map(item => item.info));
  if (command === 'create_backup') {
    const backups = JSON.parse(localStorage.getItem(previewBackupsKey) || '[]');
    const id = `backup-${Date.now()}-${Math.floor(Math.random() * 1e9)}.json`;
    const info = { id, created_ms: Date.now(), todos: state.todos.length, notes: state.notes.length, events: state.events.length };
    backups.push({ info, snapshot: JSON.parse(JSON.stringify(state)) });
    localStorage.setItem(previewBackupsKey, JSON.stringify(backups));
    return Promise.resolve(info);
  }
  if (command === 'restore_backup') {
    const backups = JSON.parse(localStorage.getItem(previewBackupsKey) || '[]');
    const backup = backups.find(item => item.info.id === args.id);
    if (!backup) throw new Error('Backup not found');
    state = JSON.parse(JSON.stringify(backup.snapshot));
    localStorage.setItem(previewKey, JSON.stringify(state));
    return Promise.resolve(state);
  }
  if (command === 'get_autostart') return Promise.resolve(false);
  if (command === 'launch_builtin' || command === 'media_key' || command === 'open_quick_link' || command === 'set_autostart') {
    throw new Error('This Windows integration requires the installed desktop application');
  }
  if (command === 'get_system_snapshot') return Promise.resolve(null); // Never manufacture telemetry in browser preview.
  const nextId = () => `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const focus = state.focus;
  const live = focusView(focus);
  if (focus.ends_at_ms != null) {
    focus.remaining_seconds = live.remaining;
    if (live.completed) { focus.ends_at_ms = null; focus.completed = true; focus.paused = false; }
  }
  if (state.countdown.ends_at_ms != null) {
    const remaining = Math.max(0, Math.ceil((state.countdown.ends_at_ms - Date.now()) / 1000));
    state.countdown.remaining_seconds = remaining;
    if (!remaining) { state.countdown.ends_at_ms = null; state.countdown.completed = true; }
  }
  if (command === 'load_data') return Promise.resolve(state);
  switch (command) {
    case 'add_todo': {
      const title = String(args.title).trim();
      if (!title || title.length > 200) throw new Error('Task title must contain 1–200 characters');
      state.todos.push({ id: nextId(), title, done: false, created_ms: Date.now() });
      break;
    }
    case 'toggle_todo': {
      const todo = state.todos.find(item => item.id === args.id);
      if (!todo) throw new Error('Task not found');
      todo.done = !todo.done; break;
    }
    case 'remove_todo': {
      if (!state.todos.some(item => item.id === args.id)) throw new Error('Task not found');
      state.todos = state.todos.filter(item => item.id !== args.id); break;
    }
    case 'add_note': state.notes.push({ id: nextId(), title: 'Untitled', body: '', updated_ms: Date.now() }); break;
    case 'update_note': {
      const note = state.notes.find(item => item.id === args.id);
      if (!note) throw new Error('Note not found');
      if (args.title.length > 200 || args.body.length > 50_000) throw new Error('Note is too long');
      Object.assign(note, { title: args.title, body: args.body, updated_ms: Date.now() }); break;
    }
    case 'remove_note': {
      if (!state.notes.some(item => item.id === args.id)) throw new Error('Note not found');
      state.notes = state.notes.filter(item => item.id !== args.id); break;
    }
    case 'add_event': {
      const title = String(args.title || '').trim();
      if (!title || title.length > 200 || !Number.isFinite(args.startsAtMs) || args.startsAtMs < 946684800000 || args.startsAtMs > 4102444800000) throw new Error('Invalid event title/date');
      if (![null, 0, 5, 15, 30, 60].includes(args.remindMinutes)) throw new Error('Invalid reminder');
      state.events.push({ id: nextId(), title, starts_at_ms: args.startsAtMs, remind_minutes: args.remindMinutes, description: args.description || '', created_ms: Date.now() });
      break;
    }
    case 'update_event': {
      const item = state.events.find(event => event.id === args.id);
      if (!item) throw new Error('Event not found');
      const title = String(args.title || '').trim();
      if (!title || title.length > 200 || !Number.isFinite(args.startsAtMs) || args.startsAtMs < 946684800000 || args.startsAtMs > 4102444800000) throw new Error('Invalid event title/date');
      if (![null, 0, 5, 15, 30, 60].includes(args.remindMinutes)) throw new Error('Invalid reminder');
      Object.assign(item, {title, starts_at_ms: args.startsAtMs, remind_minutes: args.remindMinutes, description: args.description || '', all_day: null});
      break;
    }
    case 'remove_event': {
      if (!state.events.some(item => item.id === args.id)) throw new Error('Event not found');
      state.events = state.events.filter(item => item.id !== args.id); break;
    }
    case 'add_all_day_event':
    case 'update_all_day_event': {
      const title = String(args.title || '').trim();
      const start = String(args.startDate || ''), end = String(args.endDateExclusive || '');
      const parsed = date => /^\d{4}-\d{2}-\d{2}$/.test(date) &&
        !Number.isNaN(Date.parse(date + 'T00:00:00Z')) &&
        new Date(date + 'T00:00:00Z').toISOString().slice(0, 10) === date;
      const days = (Date.parse(end + 'T00:00:00Z') - Date.parse(start + 'T00:00:00Z')) / 86400000;
      if (!title || title.length > 200 || !parsed(start) || !parsed(end) ||
          days < 1 || days > 366 || start < '2000-01-01' || end > '2100-01-01') throw new Error('Invalid all-day date span');
      const span = {start_date: start, end_date_exclusive: end};
      if (command === 'add_all_day_event') {
        state.events.push({id: nextId(), title, starts_at_ms: Date.parse(start + 'T00:00:00Z'),
          remind_minutes: null, description: args.description || '', created_ms: Date.now(), all_day: span, series_uid: null});
      } else {
        const item = state.events.find(entry => entry.id === args.id);
        if (!item) throw new Error('Event not found');
        Object.assign(item, {title, starts_at_ms: Date.parse(start + 'T00:00:00Z'),
          remind_minutes: null, description: args.description || '', all_day: span});
      }
      break;
    }
    case 'update_event_series_title':
    case 'remove_event_series': {
      const current = state.events.find(item => item.id === args.id);
      if (!current?.series_uid) throw new Error('This event is not part of an imported recurrence series');
      const members = state.events.filter(item => item.series_uid === current.series_uid);
      if (command === 'update_event_series_title') {
        const title = String(args.title || '').trim();
        if (!title || title.length > 200) throw new Error('Invalid series title');
        members.forEach(item => { item.title = title; });
      } else {
        state.events = state.events.filter(item => item.series_uid !== current.series_uid);
      }
      break;
    }
    case 'add_habit': {
      const title = String(args.title || '').trim();
      if (!title || title.length > 120) throw new Error('Invalid habit name');
      state.habits.push({id: nextId(), title, completed_dates: [], created_ms: Date.now()}); break;
    }
    case 'toggle_habit_day': {
      const habit = state.habits.find(item => item.id === args.id);
      if (!habit || !/^\d{4}-\d{2}-\d{2}$/.test(args.day)) throw new Error('Invalid habit/date');
      const i = habit.completed_dates.indexOf(args.day);
      if (i < 0) habit.completed_dates.push(args.day); else habit.completed_dates.splice(i, 1);
      break;
    }
    case 'remove_habit': {
      if (!state.habits.some(item => item.id === args.id)) throw new Error('Habit not found');
      state.habits = state.habits.filter(item => item.id !== args.id); break;
    }
    case 'add_quick_link': {
      const label = String(args.label || '').trim(), raw = String(args.url || '');
      let parsed;
      try { parsed = new URL(raw); } catch { throw new Error('Invalid quick link URL'); }
      if (!label || label.length > 120 || !['https:', 'http:'].includes(parsed.protocol) || parsed.username || parsed.password || raw.length > 2048) throw new Error('Invalid quick link');
      state.quick_links.push({id: nextId(), label, url: parsed.href, created_ms: Date.now()}); break;
    }
    case 'remove_quick_link': {
      if (!state.quick_links.some(item => item.id === args.id)) throw new Error('Quick link not found');
      state.quick_links = state.quick_links.filter(item => item.id !== args.id); break;
    }
    case 'health_action': {
      if (!/^\d{4}-\d{2}-\d{2}$/.test(args.day)) throw new Error('Invalid health date');
      if (state.health.day !== args.day) state.health = { ...defaults().health, day: args.day };
      const actions = {
        water_plus: ['water_glasses', 1, 40],
        water_minus: ['water_glasses', -1, 40],
        break_plus: ['break_count', 1, 100],
        calm_plus: ['calm_minutes', 1, 600],
      };
      const action = actions[args.action];
      if (!action) throw new Error('Unknown health action');
      state.health[action[0]] = Math.max(0, Math.min(action[2], state.health[action[0]] + action[1]));
      break;
    }
    case 'countdown_set': {
      const seconds = Number(args.seconds);
      if (!Number.isInteger(seconds) || seconds < 1 || seconds > 86400) throw new Error('Invalid countdown duration');
      state.countdown = {duration_seconds: seconds, remaining_seconds: seconds, ends_at_ms: null, completed: false};
      break;
    }
    case 'countdown_toggle': {
      const timer = state.countdown;
      if (timer.ends_at_ms != null) timer.ends_at_ms = null;
      else {
        if (!timer.remaining_seconds) timer.remaining_seconds = timer.duration_seconds;
        timer.ends_at_ms = Date.now() + timer.remaining_seconds * 1000;
        timer.completed = false;
      }
      break;
    }
    case 'countdown_adjust': {
      if (![1,5].includes(args.minutes)) throw new Error('Invalid countdown adjustment');
      const timer = state.countdown;
      timer.remaining_seconds = Math.min(86400, timer.remaining_seconds + args.minutes * 60);
      timer.duration_seconds = Math.max(timer.duration_seconds, timer.remaining_seconds);
      if (timer.ends_at_ms != null) timer.ends_at_ms = Date.now() + timer.remaining_seconds * 1000;
      timer.completed = false;
      break;
    }
    case 'countdown_reset': state.countdown = { ...defaults().countdown }; break;
    case 'set_accent': state.settings.accent = args.accent; break;
    case 'set_hover_to_open': state.settings.hover_to_open = args.enabled; break;
    case 'set_pin_open': state.settings.pin_open = args.enabled; break;
    case 'set_notch_on_top': state.settings.always_on_top = args.enabled; break;
    case 'set_fullscreen_auto': state.settings.auto_lower_fullscreen = Boolean(args.enabled); break;
    case 'set_notifications_paused': state.settings.notifications_paused = Boolean(args.paused); break;
    case 'set_notification_preferences':
      state.settings.notifications_enabled = args.enabled;
      state.settings.notifications_show_details = Boolean(args.enabled && args.showDetails);
      break;
    case 'set_focus_duration': {
      if (![15, 25, 45, 60].includes(args.minutes)) throw new Error('Invalid focus preset');
      focus.duration_seconds = args.minutes * 60;
      focus.remaining_seconds = focus.duration_seconds;
      focus.ends_at_ms = null;
      focus.completed = false; focus.paused = false; break;
    }
    case 'focus_toggle': {
      if (focus.ends_at_ms != null) { focus.ends_at_ms = null; focus.paused = true; }
      else {
        if (focus.remaining_seconds === 0) focus.remaining_seconds = focus.duration_seconds;
        focus.ends_at_ms = Date.now() + focus.remaining_seconds * 1000;
        focus.completed = false; focus.paused = false;
      }
      break;
    }
    case 'focus_reset': {
      focus.remaining_seconds = focus.duration_seconds;
      focus.ends_at_ms = null;
      focus.completed = false; focus.paused = false; break;
    }
    case 'set_expanded': return Promise.resolve(null);
    case 'close_app': throw new Error('Exit is available in the installed desktop app');
    default: throw new Error(`Unsupported command: ${command}`);
  }
  localStorage.setItem(previewKey, JSON.stringify(state));
  return Promise.resolve(state);
}

function invoke(command, args) {
  return nativeInvoke ? nativeInvoke(command, args) : previewInvoke(command, args);
}

function showToast(message, info = false) {
  const toast = $('#toast');
  toast.textContent = message;
  toast.classList.toggle('info', info);
  toast.classList.add('visible');
  clearTimeout(toastTimeout);
  toastTimeout = setTimeout(() => toast.classList.remove('visible'), 5200);
}
function showError(error) {
  console.error(error);
  showToast(error?.message || String(error));
}
function showInfo(message) { showToast(message, true); }

// Queue writes so a slower earlier IPC response cannot replace newer UI state.
function mutation(command, args = {}) {
  const task = writeQueue.catch(() => {}).then(async () => {
    data = await invoke(command, args);
    render();
    return true;
  }).catch((error) => { showError(error); return false; });
  writeQueue = task;
  return task;
}

function setExpanded(expanded) {
  const task = windowQueue.catch(() => {}).then(async () => {
    if (isExpanded === expanded) return;
    try {
      await invoke('set_expanded', { expanded });
      isExpanded = expanded;
      $('#collapsed').hidden = expanded;
      $('#expanded').hidden = !expanded;
      if (expanded) render();
    } catch (error) { showError(error); }
  });
  windowQueue = task;
  return task;
}

async function requestCollapse() {
  if (isExpanded && !(await flushNote())) return;
  await setExpanded(false);
}

async function switchTab(tab) {
  if (tab === activeTab) return;
  if (!(await flushNote())) return;
  activeTab = tab;
  $$('.tab').forEach(button => {
    const selected = button.dataset.tab === tab;
    button.classList.toggle('active', selected);
    button.setAttribute('aria-selected', String(selected));
  });
  $$('.panel').forEach(panel => panel.classList.toggle('active', panel.id === 'panel-' + tab));
  if (tab === 'settings') void refreshBackups();
  if (tab === 'system') void refreshTelemetry();
}

function node(tag, className, value) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (value != null) element.textContent = value;
  return element;
}

function renderTodos() {
  const open = data.todos.filter(item => !item.done).length;
  $('#task-count').textContent = String(open);
  $('#todo-progress').textContent = `${data.todos.length - open} of ${data.todos.length} completed`;
  const list = $('#todo-list');
  list.replaceChildren();
  if (!data.todos.length) {
    list.append(node('div', 'empty', 'No tasks yet. Add one to get started.'));
    return;
  }
  for (const item of [...data.todos].reverse()) {
    const line = node('div', 'todo-item' + (item.done ? ' done' : ''));
    const check = node('button', 'check', item.done ? '✓' : '');
    check.type = 'button';
    check.setAttribute('aria-label', `${item.done ? 'Mark incomplete' : 'Complete'}: ${item.title}`);
    check.setAttribute('aria-pressed', String(item.done));
    check.addEventListener('click', () => mutation('toggle_todo', { id: item.id }));
    const label = node('span', 'title', item.title);
    const remove = node('button', 'remove-task', '×');
    remove.type = 'button';
    remove.setAttribute('aria-label', `Delete task: ${item.title}`);
    remove.addEventListener('click', () => mutation('remove_todo', { id: item.id }));
    line.append(check, label, remove);
    list.append(line);
  }
}

function renderNotes() {
  $('#note-count').textContent = String(data.notes.length);
  const list = $('#note-list');
  list.replaceChildren();
  const query = ($('#note-search')?.value || '').trim().toLocaleLowerCase();
  const sorted = [...data.notes]
    .filter(note => !query || (note.title + ' ' + note.body).toLocaleLowerCase().includes(query))
    .sort((left, right) => right.updated_ms - left.updated_ms);
  for (const note of sorted) {
    const button = node('button', 'note-card' + (selectedNoteId === note.id ? ' selected' : ''), note.title || 'Untitled');
    button.type = 'button';
    button.append(node('small', '', note.body.split('\n').find(text => text.trim()) || 'Empty note'));
    button.addEventListener('click', async () => {
      if (!(await flushNote())) return;
      selectedNoteId = note.id;
      renderNotes();
    });
    list.append(button);
  }
  if (!sorted.length) list.append(node('div', 'empty', query ? 'No matching notes.' : 'Nothing saved yet.'));
  const current = data.notes.find(note => note.id === selectedNoteId);
  for (const control of [$('#note-title'), $('#note-body'), $('#note-delete'), $('#note-save')]) {
    control.disabled = !current;
  }
  if (boundEditorId !== selectedNoteId) {
    $('#note-title').value = current?.title || '';
    $('#note-body').value = current?.body || '';
    boundEditorId = selectedNoteId;
  }
  const message = !current ? 'Choose or create a note'
    : pendingDraft ? 'Unsaved changes' : 'Saved locally';
  $('#save-status').textContent = message;
}

function eventDate(epoch) {
  return new Date(epoch).toLocaleString(undefined, { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' });
}
function asLocalInput(epoch) {
  const d = new Date(epoch);
  const pad = n => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
function setAllDayEditor(enabled) {
  $('#event-all-day').checked = enabled;
  $('#event-all-day-fields').hidden = !enabled;
  $('#event-time').hidden = enabled;
  $('#event-time').required = !enabled;
  $('#event-remind').disabled = enabled;
  if (enabled) $('#event-remind').value = 'off';
  if (enabled && !$('#event-start-date').value) {
    const date = ($('#event-time').value || asLocalInput(Date.now())).slice(0, 10);
    $('#event-start-date').value = date;
    const next = new Date(date + 'T12:00:00');
    next.setDate(next.getDate() + 1);
    const pad = n => String(n).padStart(2, '0');
    $('#event-end-date').value = [next.getFullYear(), pad(next.getMonth() + 1), pad(next.getDate())].join('-');
  }
}
function eventDisplayDate(event) {
  if (!event.all_day) return eventDate(event.starts_at_ms);
  const end = new Date(Date.parse(event.all_day.end_date_exclusive + 'T00:00:00Z') - 86400000)
    .toISOString().slice(0, 10);
  return event.all_day.start_date === end ? end + ' · All day'
    : event.all_day.start_date + ' – ' + end + ' · All day';
}
function cancelEventEdit() {
  selectedEventId = null;
  selectedSeriesEventId = null;
  $('#event-submit').textContent = 'Add event';
  $('#event-cancel').hidden = true;
  $('#event-title').value = '';
  $('#event-time').disabled = false;
  $('#event-all-day').disabled = false;
  setAllDayEditor(false);
}
function renderCalendar() {
  const events = [...data.events].sort((a, b) => a.starts_at_ms - b.starts_at_ms);
  $('#calendar-count').textContent = `${events.length} EVENT${events.length === 1 ? '' : 'S'}`;
  const localDay = asLocalInput(Date.now()).slice(0, 10);
  const upcoming = events.find(item => item.all_day
    ? item.all_day.end_date_exclusive > localDay
    : item.starts_at_ms >= Date.now() - 60000);
  $('#next-event').textContent = upcoming ? upcoming.title + ' · ' + eventDisplayDate(upcoming) : 'No upcoming events';
  const list = $('#event-list');
  list.replaceChildren();
  if (!events.length) {
    list.append(node('div', 'empty', 'Your calendar is clear. Create your first event.'));
    return;
  }
  for (const event of events) {
    const past = event.all_day ? event.all_day.end_date_exclusive <= localDay : event.starts_at_ms < Date.now();
    const line = node('div', 'event-item' + (past ? ' past' : ''));
    const date = node('time', 'event-date', eventDisplayDate(event));
    date.dateTime = event.all_day ? event.all_day.start_date : new Date(event.starts_at_ms).toISOString();
    const info = node('div', 'event-info');
    info.append(node('b', '', event.title));
    info.append(node('small', '', event.all_day ? 'All day · no midnight reminder'
      : (event.remind_minutes == null ? 'Reminder off' : (event.remind_minutes === 0 ? 'At event time' : String(event.remind_minutes) + ' min before')));
    const edit = node('button', 'text-button', 'Edit');
    edit.type = 'button';
    edit.setAttribute('aria-label', `Edit event: ${event.title}`);
    edit.addEventListener('click', () => {
      cancelEventEdit();
      selectedEventId = event.id;
      $('#event-title').value = event.title;
      $('#event-time').value = asLocalInput(event.starts_at_ms);
      if (event.all_day) {
        $('#event-start-date').value = event.all_day.start_date;
        $('#event-end-date').value = event.all_day.end_date_exclusive;
        setAllDayEditor(true);
      }
      $('#event-remind').value = event.remind_minutes == null ? 'off' : String(event.remind_minutes);
      $('#event-submit').textContent = 'Save changes';
      $('#event-cancel').hidden = false;
      $('#event-title').focus();
    });
    const remove = node('button', 'remove-task', '×');
    remove.type = 'button';
    remove.title = `Remove ${event.title}`;
    remove.setAttribute('aria-label', `Delete event: ${event.title}`);
    remove.addEventListener('click', async () => {
      if (await mutation('remove_event', { id: event.id }) && selectedEventId === event.id) cancelEventEdit();
    });
    line.append(date, info, edit, remove);
    if (event.series_uid) {
      const rename = node('button', 'text-button', 'Rename series');
      rename.type = 'button';
      rename.setAttribute('aria-label', 'Rename entire series: ' + event.title);
      rename.addEventListener('click', () => {
        cancelEventEdit();
        selectedSeriesEventId = event.id;
        $('#event-title').value = event.title;
        $('#event-time').disabled = true;
        $('#event-time').required = false;
        $('#event-all-day').disabled = true;
        $('#event-submit').textContent = 'Rename entire series';
        $('#event-cancel').hidden = false;
        $('#event-title').focus();
      });
      const deleteSeries = node('button', 'text-button', 'Delete series');
      deleteSeries.type = 'button';
      deleteSeries.setAttribute('aria-label', 'Delete entire series: ' + event.title);
      deleteSeries.addEventListener('click', async () => {
        if (!window.confirm('Delete all local occurrences of this imported series? Re-import can restore them.')) return;
        if (await mutation('remove_event_series', {id: event.id})) cancelEventEdit();
      });
      line.append(rename, deleteSeries);
    }
    list.append(line);
  }
}

function renderBackups() {
  const list = $('#backup-list');
  list.replaceChildren();
  if (!backupList.length) {
    list.append(node('div', 'empty', 'No backups yet. Create a snapshot to enable restore.'));
    return;
  }
  for (const backup of [...backupList].sort((a, b) => b.created_ms - a.created_ms)) {
    const line = node('div', 'backup-entry');
    const label = node('div', '', new Date(backup.created_ms).toLocaleString());
    label.append(node('small', '', `${backup.todos} tasks · ${backup.notes} notes · ${backup.events} events`));
    const restore = node('button', 'secondary', 'Restore');
    restore.type = 'button';
    restore.setAttribute('aria-label', `Restore backup ${backup.id}`);
    restore.addEventListener('click', async () => {
      if (!window.confirm('Restore this backup? Current tasks, notes, calendar and settings will be replaced. The current state is retained in the automatic .bak file.')) return;
      if (!(await flushNote())) return;
      await writeQueue;
      try {
        data = await invoke('restore_backup', { id: backup.id });
        selectedNoteId = null; boundEditorId = null; pendingDraft = null;
        cancelEventEdit();
        render();
        showInfo('Backup restored successfully.');
      } catch (error) { showError(error); }
    });
    line.append(label, restore);
    list.append(line);
  }
}
async function refreshBackups() {
  try { backupList = await invoke('list_backups'); renderBackups(); }
  catch (error) { showError(error); }
}

async function refreshTelemetry() {
  if (telemetryPending || activeTab !== 'system') return;
  telemetryPending = true;
  try {
    const sample = await invoke('get_system_snapshot');
    if (!sample || !sample.memory_total_bytes) {
      $('#system-detail').textContent = 'System data is available in the native Rust/Windows app only.';
      return;
    }
    const percentage = Math.min(100, 100 * sample.memory_used_bytes / sample.memory_total_bytes);
    $('#system-cpu').textContent = `${Math.round(sample.cpu_percent)}%`;
    $('#system-memory').textContent = `${Math.round(percentage)}%`;
    $('#cpu-meter').style.width = `${Math.max(0, Math.min(100, sample.cpu_percent))}%`;
    $('#memory-meter').style.width = `${percentage}%`;
    const gb = bytes => (bytes / (1024 ** 3)).toFixed(1);
    $('#system-detail').textContent = `RAM ${gb(sample.memory_used_bytes)} / ${gb(sample.memory_total_bytes)} GiB · sampled ${new Date(sample.sampled_at_ms).toLocaleTimeString()}`;
  } catch (error) { showError(error); }
  finally { telemetryPending = false; telemetryLastAt = Date.now(); }
}
function checkReminders(now = Date.now()) {
  // In-app reminder remains available; native OS toast requires explicit opt-in.
  if (document.hidden) return;
  for (const event of data.events) {
    if (event.remind_minutes == null) continue;
    const due = event.starts_at_ms - event.remind_minutes * 60000;
    if (now >= due && now < due + 10 * 60000 && !reminderSeen.has(event.id)) {
      reminderSeen.add(event.id);
      showInfo(`Calendar reminder: ${event.title} · ${eventDate(event.starts_at_ms)}`);
      // Native OS reminders are dispatched by the Rust scheduler, not this WebView.
      // The in-app banner is intentionally independent of OS notification consent.
      break; // Show at most one reminder at a time.
    }
  }
}

function renderSettings() {
  const accent = data.settings.accent || '#d6b77a';
  document.documentElement.style.setProperty('--accent', accent);
  $$('.swatch').forEach(button => {
    const chosen = button.dataset.color === accent;
    button.classList.toggle('chosen', chosen);
    button.setAttribute('aria-pressed', String(chosen));
  });
  $('#hover-toggle').checked = data.settings.hover_to_open;
  $('#pin-toggle').checked = data.settings.pin_open;
  $('#on-top-toggle').checked = data.settings.always_on_top !== false;
  $('#fullscreen-auto-toggle').checked = Boolean(data.settings.auto_lower_fullscreen);
  $('#notifications-toggle').checked = Boolean(data.settings.notifications_enabled);
  $('#notification-details-toggle').checked = Boolean(data.settings.notifications_show_details);
  $('#notification-details-toggle').disabled = !data.settings.notifications_enabled;
  $('#notifications-pause-toggle').checked = Boolean(data.settings.notifications_paused);
  $('#notifications-pause-toggle').disabled = !data.settings.notifications_enabled;
  $('#notification-test').disabled = !nativeInvoke || !data.settings.notifications_enabled || Boolean(data.settings.notifications_paused);
  $('#notification-scheduler-refresh').disabled = !nativeInvoke;
  $('#notification-permission-check').disabled = !nativeInvoke;
  const pinButton = $('#pin-button');
  pinButton.setAttribute('aria-pressed', String(data.settings.pin_open));
  pinButton.classList.toggle('on', data.settings.pin_open);
  pinButton.title = data.settings.pin_open ? 'Unpin panel' : 'Keep panel open';
}

function formatted(seconds) {
  const value = Math.max(0, Math.ceil(seconds));
  return `${String(Math.floor(value / 60)).padStart(2, '0')}:${String(value % 60).padStart(2, '0')}`;
}

function renderTimer() {
  const focus = data.focus || defaults().focus;
  const view = focusView(focus);
  const display = formatted(view.remaining);
  $('#timer-display').textContent = display;
  $('#focus-status').textContent = display;
  $('.timer-ring').style.setProperty('--timer-progress', `${100 * view.remaining / focus.duration_seconds}%`);
  $('#timer-start').textContent = view.running ? 'Pause focus' : view.completed ? 'Start again' : focus.paused ? 'Resume focus' : 'Start focus';
  $('#timer-label').textContent = view.completed ? 'SESSION COMPLETE' : view.running ? 'STAY IN THE ZONE' : focus.paused ? 'PAUSED' : 'READY WHEN YOU ARE';
  $$('[data-minutes]').forEach(button => {
    const chosen = Number(button.dataset.minutes) * 60 === focus.duration_seconds;
    button.classList.toggle('selected', chosen);
    button.setAttribute('aria-pressed', String(chosen));
  });
}

function render() {
  renderSettings();
  renderTodos();
  renderNotes();
  renderCalendar();
  if (typeof renderCalendarPlus === 'function') renderCalendarPlus();
  renderTimer();
  if (typeof renderExtras === 'function') renderExtras();
  if (typeof renderParity === 'function') renderParity();
}

async function flushNote() {
  clearTimeout(noteSaveTimeout);
  noteSaveTimeout = null;
  if (!pendingDraft) return noteSaveInFlight ? noteSaveInFlight : true;
  const draft = pendingDraft;
  pendingDraft = null;
  $('#save-status').textContent = 'Saving…';
  const previous = noteSaveInFlight;
  const job = (async () => {
    if (previous && !(await previous)) return false;
    const success = await mutation('update_note', draft);
    if (!success && !pendingDraft) {
      pendingDraft = draft;
      $('#save-status').textContent = 'Save failed — retry';
    }
    return success;
  })();
  noteSaveInFlight = job;
  try { return await job; }
  finally { if (noteSaveInFlight === job) noteSaveInFlight = null; }
}

function scheduleNoteSave() {
  if (!selectedNoteId) return;
  pendingDraft = {
    id: selectedNoteId,
    title: $('#note-title').value,
    body: $('#note-body').value,
  };
  $('#save-status').textContent = 'Unsaved changes';
  clearTimeout(noteSaveTimeout);
  noteSaveTimeout = setTimeout(() => { void flushNote(); }, 650);
}

function clockTick() {
  const now = new Date();
  const text = now.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  $('#closed-clock').textContent = text;
  $('#hero-time').textContent = text;
  $('#current-date').textContent = now.toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' });
  renderTimer();
  if (data.focus.completed && !focusCompletionNotified && !document.hidden) {
    focusCompletionNotified = true;
    showInfo('Focus session complete. Great work!');
  }
  if (!data.focus.completed) focusCompletionNotified = false;
  checkReminders(now.getTime());
  if (typeof tickParity === 'function') tickParity();
  if (activeTab === 'system' && Date.now() - telemetryLastAt > 4000) void refreshTelemetry();
}

function bind() {
  $('#collapsed').addEventListener('mouseenter', () => {
    if (data.settings.hover_to_open) void setExpanded(true);
  });
  $('#collapsed').addEventListener('click', () => void setExpanded(true));
  $('#collapsed').addEventListener('keydown', event => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault(); void setExpanded(true);
    }
  });
  $('#notch').addEventListener('mouseleave', () => {
    if (nativeInvoke && data.settings.hover_to_open && !data.settings.pin_open) void requestCollapse();
  });
  $('#collapse').addEventListener('click', () => void requestCollapse());
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape' && isExpanded) void requestCollapse();
  });
  $$('.tab').forEach(button => button.addEventListener('click', () => void switchTab(button.dataset.tab)));
  $('#go-timer').addEventListener('click', () => void switchTab('timer'));
  $('#go-calendar').addEventListener('click', () => void switchTab('calendar'));
  $('#event-form').addEventListener('submit', async event => {
    event.preventDefault();
    const title = $('#event-title').value.trim();
    const raw = $('#event-time').value;
    const startsAtMs = new Date(raw).getTime();
    if (!title || !Number.isFinite(startsAtMs)) { showError('Choose a valid event title and date'); return; }
    const remind = $('#event-remind').value;
    const mode = selectedEventId ? 'update_event' : 'add_event';
    if (await mutation(mode, {
      ...(selectedEventId ? { id: selectedEventId } : {}),
      title, startsAtMs,
      description: '',
      remindMinutes: remind === 'off' ? null : Number(remind),
    })) {
      cancelEventEdit();
      showInfo('Calendar event saved locally.');
    }
  });
  $('#event-cancel').addEventListener('click', cancelEventEdit);
  $('#system-refresh').addEventListener('click', () => void refreshTelemetry());
  $('#backup-create').addEventListener('click', async () => {
    if (!(await flushNote())) return;
    await writeQueue;
    const button = $('#backup-create');
    button.disabled = true;
    try {
      await invoke('create_backup');
      await refreshBackups();
      showInfo('Local backup created.');
    } catch (error) { showError(error); }
    finally { button.disabled = false; }
  });
  $('#todo-form').addEventListener('submit', async event => {
    event.preventDefault();
    const input = $('#todo-input');
    const title = input.value.trim();
    if (title && await mutation('add_todo', { title })) input.value = '';
  });
  $('#note-new').addEventListener('click', async () => {
    if (!(await flushNote())) return;
    const previous = new Set(data.notes.map(note => note.id));
    if (await mutation('add_note')) {
      selectedNoteId = data.notes.find(note => !previous.has(note.id))?.id || null;
      renderNotes();
      $('#note-title').focus();
      $('#note-title').select();
    }
  });
  $('#note-title').addEventListener('input', scheduleNoteSave);
  $('#note-body').addEventListener('input', scheduleNoteSave);
  $('#note-title').addEventListener('blur', () => { void flushNote(); });
  $('#note-body').addEventListener('blur', () => { void flushNote(); });
  $('#note-save').addEventListener('click', () => { void flushNote(); });
  $('#note-delete').addEventListener('click', async () => {
    if (!selectedNoteId) return;
    clearTimeout(noteSaveTimeout);
    noteSaveTimeout = null;
    const id = selectedNoteId;
    const draft = pendingDraft;
    pendingDraft = null;
    if (await mutation('remove_note', { id })) {
      selectedNoteId = null;
      renderNotes();
    } else { pendingDraft = draft; }
  });
  $$('.swatch').forEach(button => button.addEventListener('click', () => void mutation('set_accent', { accent: button.dataset.color })));
  $('#hover-toggle').addEventListener('change', event => void mutation('set_hover_to_open', { enabled: event.target.checked }));
  $('#pin-toggle').addEventListener('change', event => void mutation('set_pin_open', { enabled: event.target.checked }));
  $('#on-top-toggle').addEventListener('change', event => void mutation('set_notch_on_top', { enabled: event.target.checked }));
  $('#fullscreen-auto-toggle').addEventListener('change', event => void mutation('set_fullscreen_auto', { enabled: event.target.checked }));
  $('#notifications-toggle').addEventListener('change', event => void mutation('set_notification_preferences', { enabled: event.target.checked, showDetails: event.target.checked && $('#notification-details-toggle').checked }));
  $('#notification-details-toggle').addEventListener('change', event => void mutation('set_notification_preferences', { enabled: $('#notifications-toggle').checked, showDetails: event.target.checked }));
  $('#notifications-pause-toggle').addEventListener('change', event => void mutation('set_notifications_paused', { paused: event.target.checked }));
  $('#notification-scheduler-refresh').addEventListener('click', async () => {
    if (!nativeInvoke) return;
    try {
      const status = await invoke('get_reminder_scheduler_status');
      const state = !status.enabled ? 'Disabled' : status.paused ? 'Paused' : 'Active';
      const count = Number(status.scheduled_count || 0);
      const upcoming = Number(status.next_due_at_ms);
      const next = Number.isFinite(upcoming) && upcoming > 0
        ? ' Next reminder: ' + new Date(upcoming).toLocaleString() + '.'
        : '';
      $('#notification-scheduler-status').textContent =
        'Native reminder scheduler: ' + state + '. ' + count +
        ' upcoming reminder(s); checks every ' + status.poll_interval_seconds +
        ' seconds while WFS Notch is running.' + next;
    } catch (error) {
      $('#notification-scheduler-status').textContent = 'Native scheduler status unavailable.';
      showError(error);
    }
  });
  $('#notification-permission-check').addEventListener('click', async () => {
    if (!nativeInvoke) return;
    try {
      const permission = await invoke('get_windows_notification_permission');
      const labels = {
        enabled: 'Windows reports toasts enabled for WFS Notch.',
        disabled_for_application: 'Notifications are disabled for WFS Notch in Windows Settings.',
        disabled_for_user: 'Windows notifications are disabled for this user.',
        disabled_by_policy: 'Windows policy blocks notifications.',
        disabled_by_manifest: 'Application toast capability is not enabled.',
        unavailable: 'Windows could not report app permission (portable or unregistered app).',
      };
      $('#notification-permission-status').textContent =
        (labels[permission.status] || 'Unknown Windows notification state.') +
        ' Focus Assist or Do Not Disturb can still suppress delivery.';
    } catch (error) {
      $('#notification-permission-status').textContent = 'Cannot inspect Windows notification setting.';
      showError(error);
    }
  });
  $('#notification-test').addEventListener('click', async () => {
    if (!nativeInvoke || !data.settings.notifications_enabled) return;
    try {
      await invoke('notify_test');
      $('#notification-status').textContent = 'Test requested. Check Windows notification settings if no toast appears.';
    } catch (error) {
      $('#notification-status').textContent = 'Windows notification could not be delivered.';
      showError(error);
    }
  });
  $('#pin-button').addEventListener('click', () => void mutation('set_pin_open', { enabled: !data.settings.pin_open }));
  $$('[data-minutes]').forEach(button => button.addEventListener('click', () => void mutation('set_focus_duration', { minutes: Number(button.dataset.minutes) })));
  $('#timer-start').addEventListener('click', () => void mutation('focus_toggle'));
  $('#timer-reset').addEventListener('click', () => void mutation('focus_reset'));
  $('#app-exit').addEventListener('click', async () => {
    if (!(await flushNote())) return;
    try { await invoke('close_app'); } catch (error) { showError(error); }
  });
  document.addEventListener('visibilitychange', () => {
    if (document.hidden && pendingDraft) void flushNote();
  });
}

async function start() {
  bind();
  if (typeof initExtras === 'function') initExtras();
  if (!nativeInvoke) {
    document.body.classList.add('preview');
    $('#environment').textContent = 'BROWSER PREVIEW · MOCK STORAGE';
  }
  try {
    data = await invoke('load_data');
  } catch (error) { showError(error); }
  data.settings = { ...defaults().settings, ...data.settings };
  data.focus = { ...defaults().focus, ...data.focus };
  data.events = Array.isArray(data.events) ? data.events : [];
  data.habits = Array.isArray(data.habits) ? data.habits : [];
  data.quick_links = Array.isArray(data.quick_links) ? data.quick_links : [];
  data.health = { ...defaults().health, ...data.health };
  data.countdown = { ...defaults().countdown, ...data.countdown };
  $('#event-time').value = asLocalInput(Date.now() + 15 * 60000);
  if (!nativeInvoke || data.settings.pin_open) await setExpanded(true);
  render();
  void refreshBackups();
  clockTick();
  setInterval(clockTick, 250);
}

void start();
