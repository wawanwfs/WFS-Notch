'use strict';

const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => [...document.querySelectorAll(selector)];
const nativeInvoke = window.__TAURI__?.core?.invoke;
const previewKey = 'wfs-notch-browser-preview-v2';
const previewBackupsKey = 'wfs-notch-browser-backups-v3';
const noteDraftKey = 'wfs-notch-local-note-draft-v1';
const defaults = () => ({
  todos: [], notes: [], events: [], calendar_tombstones: [], calendar_series: {}, reminder_retries: {}, habits: [], quick_links: [],
  health: { day: '', water_glasses: 0, break_count: 0, calm_minutes: 0 },
  countdown: { duration_seconds: 300, remaining_seconds: 300, ends_at_ms: null, completed: false },
  settings: { accent: '#d6b77a', hover_to_open: true, pin_open: false, monitor_id: 'primary', top_margin_px: 12, notifications_enabled: false, notifications_show_details: false, notifications_paused: false, task_due_reminders_enabled: false, always_on_top: true, auto_lower_fullscreen: false, ui_density: 'comfortable', reduce_motion: false, high_contrast: false },
  focus: { duration_seconds: 1500, remaining_seconds: 1500, ends_at_ms: null, completed: false, paused: false, cycle_enabled: false, phase: 'focus', completed_in_cycle: 0 },
});
let data = defaults();
let selectedNoteId = null;
let selectedTodoId = null;
let selectedEventId = null;
let selectedSeriesEventId = null;
let boundEditorId = null;
let pendingDraft = null;
let noteSaveTimeout = null;
let noteSaveInFlight = null;
let noteConflictDraft = null;
let toastTimeout = null;
let activeTab = 'home';
let tabRequest = 0;
let backupList = [];
let reminderSeen = new Set();
let focusCompletionNotified = false;
let focusAdvancePending = false;
let focusAdvanceRetryAt = 0;
let telemetryPending = false;
let telemetryLastAt = 0;
let isExpanded = false;
let windowQueue = Promise.resolve();
let writeQueue = Promise.resolve();

function validTimedEnd(start, end) {
  return end == null || (Number.isSafeInteger(end) && end > start &&
    end <= 4102444800000 && end - start <= 366 * 86400000);
}

function normalizeDraftTags(raw) {
  if (!Array.isArray(raw) || raw.length > 8) throw new Error('A note supports at most 8 tags');
  const result = [];
  for (const value of raw) {
    if (typeof value !== 'string') throw new Error('Invalid note tag');
    const tag = value.trim().toLowerCase();
    if (!/^[a-z0-9_-]{1,24}$/.test(tag))
      throw new Error('Each tag must be 1–24 ASCII letters, numbers, - or _');
    if (!result.includes(tag)) result.push(tag);
  }
  return result;
}

function persistPendingNote(draft) {
  try {
    if (draft) localStorage.setItem(noteDraftKey, JSON.stringify(draft));
    else localStorage.removeItem(noteDraftKey);
    return true;
  } catch (error) {
    console.warn('Unable to persist an unsaved local note draft', error?.message);
    return false;
  }
}
function readPendingNote() {
  try {
    const raw = localStorage.getItem(noteDraftKey);
    if (!raw || raw.length > 120000) return null;
    const draft = JSON.parse(raw);
    if (!draft || typeof draft.id !== 'string' || draft.id.length > 128 ||
      typeof draft.title !== 'string' || draft.title.length > 200 ||
      typeof draft.body !== 'string' || draft.body.length > 50000 ||
      !Number.isSafeInteger(draft.expectedRevision) || draft.expectedRevision < 0) return null;
    normalizeDraftTags(draft.tags);
    return draft;
  } catch (_) { return null; }
}


const notesImportFormat = 'wfs-notch-notes';
let notesImportPreview = null;
let notesImportPayload = null;
function parseNotesTransfer(payload, existing) {
  if (new TextEncoder().encode(payload).length > 1024 * 1024)
    throw new Error('Notes JSON exceeds the 1 MiB limit');
  let parsed;
  try { parsed = JSON.parse(payload); }
  catch (_) { throw new Error('Invalid Notes JSON'); }
  if (!parsed || parsed.format !== notesImportFormat || parsed.version !== 1 ||
    !Array.isArray(parsed.notes)) throw new Error('Unsupported Notes JSON format/version');
  if (parsed.notes.length > 250) throw new Error('A Notes import supports at most 250 entries');
  const seen = new Set();
  for (const note of parsed.notes) {
    if (!note || typeof note.id !== 'string' || !/^[a-zA-Z0-9_-]{1,128}$/.test(note.id) ||
      seen.has(note.id)) throw new Error('Notes JSON contains duplicate or invalid IDs');
    seen.add(note.id);
    if (typeof note.title !== 'string' || [...note.title].length > 200 ||
      typeof note.body !== 'string' || [...note.body].length > 50000 ||
      !Number.isSafeInteger(note.updated_ms) || note.updated_ms < 0 ||
      !Number.isSafeInteger(note.revision ?? 0) || (note.revision ?? 0) > 1e9)
      throw new Error('Notes JSON contains invalid content/revision');
    const tags = normalizeDraftTags(note.tags || []);
    if (JSON.stringify(tags) !== JSON.stringify(note.tags || []))
      throw new Error('Notes JSON tags must be canonical');
    if (Object.keys(note).some(key => !['id','title','body','updated_ms','tags','revision'].includes(key)))
      throw new Error('Notes JSON contains unsupported fields');
  }
  const known = new Set(existing.map(note => note.id));
  const newCount = parsed.notes.filter(note => !known.has(note.id)).length;
  if (existing.length + newCount > 500) throw new Error('Notes import exceeds the 500 note safety limit');
  return {notes:parsed.notes, incoming:parsed.notes.length,
    new_count:newCount, existing_count:parsed.notes.length - newCount};
}

function focusPhaseSeconds(focus) {
  return focus.phase === 'short_break' ? 300 : focus.phase === 'long_break' ? 900 : focus.duration_seconds;
}

function focusView(focus, current = Date.now()) {
  const remaining = focus.ends_at_ms == null
    ? focus.remaining_seconds
    : Math.min(focusPhaseSeconds(focus), Math.max(0, Math.ceil((focus.ends_at_ms - current) / 1000)));
  return {
    remaining,
    running: focus.ends_at_ms != null && remaining > 0,
    completed: (focus.ends_at_ms != null && remaining === 0) || (focus.completed && remaining === 0),
  };
}

function normalizePreviewFocus(focus, now) {
  if (focus.ends_at_ms == null) return;
  if (focus.cycle_enabled && focus.phase === 'focus' && focus.ends_at_ms <= now) {
    focus.completed_in_cycle = Math.min(4, focus.completed_in_cycle + 1);
    focus.phase = focus.completed_in_cycle === 4 ? 'long_break' : 'short_break';
    focus.ends_at_ms += focusPhaseSeconds(focus) * 1000;
  }
  if (focus.cycle_enabled && focus.ends_at_ms <= now) {
    if (focus.phase === 'long_break') focus.completed_in_cycle = 0;
    focus.phase = 'focus';
    Object.assign(focus, {remaining_seconds: focusPhaseSeconds(focus), ends_at_ms: null, completed: false, paused: false});
    return;
  }
  const live = focusView(focus, now);
  focus.remaining_seconds = live.remaining;
  if (live.completed) Object.assign(focus, {ends_at_ms: null, completed: true, paused: false});
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
  state.calendar_tombstones = Array.isArray(state.calendar_tombstones) ? state.calendar_tombstones : [];
  state.calendar_series = state.calendar_series && typeof state.calendar_series === 'object' && !Array.isArray(state.calendar_series)
    ? state.calendar_series : {};
  state.reminder_retries = state.reminder_retries && typeof state.reminder_retries === 'object' && !Array.isArray(state.reminder_retries)
    ? state.reminder_retries : {};
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
    const restored = JSON.parse(JSON.stringify(backup.snapshot));
    const currentRevisions = new Map(state.notes.map(note =>
      [note.id, note.revision || 0]));
    for (const note of restored.notes) {
      if (!currentRevisions.has(note.id)) continue;
      const revision = Math.max(note.revision || 0, currentRevisions.get(note.id));
      if (!Number.isSafeInteger(revision + 1)) throw new Error('Cannot restore: note revision limit exceeded');
      note.revision = revision + 1;
    }
    state = restored;
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
  const focusNow = Date.now();
  const focusWasDue = focus.cycle_enabled && focus.ends_at_ms != null && focus.ends_at_ms <= focusNow;
  normalizePreviewFocus(focus, focusNow);
  if (state.countdown.ends_at_ms != null) {
    const remaining = Math.max(0, Math.ceil((state.countdown.ends_at_ms - Date.now()) / 1000));
    state.countdown.remaining_seconds = remaining;
    if (!remaining) { state.countdown.ends_at_ms = null; state.countdown.completed = true; }
  }
  if (command === 'load_data' || (command === 'advance_focus_cycle' && !focusWasDue)) {
    if (focusWasDue) localStorage.setItem(previewKey, JSON.stringify(state));
    return Promise.resolve(state);
  }
  if (command === 'export_notes_json') {
    if (state.notes.length > 250) throw new Error('Export supports at most 250 notes');
    const json = JSON.stringify({format:notesImportFormat, version:1, notes:state.notes},null,2);
    parseNotesTransfer(json, []);
    return Promise.resolve(json);
  }
  if (command === 'preview_notes_import') {
    const checked = parseNotesTransfer(args.payload, state.notes);
    return Promise.resolve({incoming:checked.incoming, new_count:checked.new_count,
      existing_count:checked.existing_count});
  }
  switch (command) {
    case 'add_todo': {
      const title = String(args.title).trim();
      const priority = args.priority || 'normal';
      if (!title || title.length > 200) throw new Error('Task title must contain 1–200 characters');
      if (!['low', 'normal', 'high'].includes(priority)) throw new Error('Invalid task priority');
      if (args.dueAtMs != null && (!Number.isSafeInteger(args.dueAtMs) || args.dueAtMs < 946684800000 || args.dueAtMs > 4102444800000)) throw new Error('Invalid task due time');
      state.todos.push({ id: nextId(), title, done: false, created_ms: Date.now(), priority, due_at_ms: args.dueAtMs ?? null });
      break;
    }
    case 'update_todo': {
      const todo = state.todos.find(item => item.id === args.id);
      if (!todo) throw new Error('Task not found');
      const title = String(args.title || '').trim();
      if (!title || title.length > 200 || !['low', 'normal', 'high'].includes(args.priority))
        throw new Error('Invalid task edit');
      if (args.dueAtMs != null && (!Number.isSafeInteger(args.dueAtMs) || args.dueAtMs < 946684800000 || args.dueAtMs > 4102444800000)) throw new Error('Invalid task due time');
      Object.assign(todo, { title, priority: args.priority, due_at_ms: args.dueAtMs ?? null });
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
    case 'add_note': state.notes.push({ id: nextId(), title: 'Untitled', body: '', updated_ms: Date.now(), tags: [], revision: 0 }); break;
    case 'import_notes_json': {
      const checked = parseNotesTransfer(args.payload, state.notes);
      const known = new Set(state.notes.map(note => note.id));
      state.notes.push(...checked.notes.filter(note => !known.has(note.id)));
      break;
    }
    case 'create_note_from_draft': {
      if (String(args.title).length > 200 || String(args.body).length > 50_000) throw new Error('Note is too long');
      const tags = normalizeDraftTags(args.tags);
      state.notes.push({id:nextId(), title:args.title, body:args.body, tags,
        updated_ms:Date.now(), revision:0});
      break;
    }
    case 'update_note': {
      const note = state.notes.find(item => item.id === args.id);
      if (!note) throw new Error('Note not found');
      if ((note.revision || 0) !== args.expectedRevision) throw new Error('NOTE_CONFLICT: this note changed since the editor loaded it');
      if (args.title.length > 200 || args.body.length > 50_000) throw new Error('Note is too long');
      const tags = args.tags == null ? (note.tags || []) : normalizeDraftTags(args.tags);
      const same = note.title === args.title && note.body === args.body &&
        JSON.stringify(note.tags || []) === JSON.stringify(tags);
      if (!same) Object.assign(note, {title:args.title, body:args.body, tags,
        revision:(note.revision || 0) + 1, updated_ms:Date.now()});
      break;
    }
    case 'remove_note': {
      const note = state.notes.find(item => item.id === args.id);
      if (!note) throw new Error('Note not found');
      if ((note.revision || 0) !== args.expectedRevision) throw new Error('NOTE_CONFLICT: note changed before deletion');
      state.notes = state.notes.filter(item => item.id !== args.id); break;
    }
    case 'add_event': {
      const title = String(args.title || '').trim();
      if (!title || title.length > 200 || !Number.isFinite(args.startsAtMs) || args.startsAtMs < 946684800000 || args.startsAtMs > 4102444800000) throw new Error('Invalid event title/date');
      if (![null, 0, 5, 15, 30, 60].includes(args.remindMinutes)) throw new Error('Invalid reminder');
      if (!validTimedEnd(args.startsAtMs, args.endsAtMs)) throw new Error('Timed end must be after start and within 366 days');
      if (String(args.description || '').length > 2000) throw new Error('Description exceeds 2000 characters');
      state.events.push({ id: nextId(), title, starts_at_ms: args.startsAtMs, ends_at_ms: args.endsAtMs ?? null, remind_minutes: args.remindMinutes, description: args.description || '', created_ms: Date.now() });
      break;
    }
    case 'update_event': {
      const item = state.events.find(event => event.id === args.id);
      if (!item) throw new Error('Event not found');
      const title = String(args.title || '').trim();
      if (!title || title.length > 200 || !Number.isFinite(args.startsAtMs) || args.startsAtMs < 946684800000 || args.startsAtMs > 4102444800000) throw new Error('Invalid event title/date');
      if (![null, 0, 5, 15, 30, 60].includes(args.remindMinutes)) throw new Error('Invalid reminder');
      if (!validTimedEnd(args.startsAtMs, args.endsAtMs)) throw new Error('Timed end must be after start and within 366 days');
      if (String(args.description || '').length > 2000) throw new Error('Description exceeds 2000 characters');
      Object.assign(item, {title, starts_at_ms: args.startsAtMs, ends_at_ms: args.endsAtMs ?? null, remind_minutes: args.remindMinutes, description: args.description || '', all_day: null});
      const series = state.calendar_series[item.series_uid];
      if (series) {
        if (!Array.isArray(series.modified_occurrence_ids)) series.modified_occurrence_ids = [];
        if (!series.modified_occurrence_ids.includes(item.id)) series.modified_occurrence_ids.push(item.id);
      }
      break;
    }
    case 'remove_event': {
      const found = state.events.find(item => item.id === args.id);
      if (!found) throw new Error('Event not found');
      if (found.id.startsWith('ics:') || found.series_uid) {
        if (!state.calendar_tombstones.includes(found.id)) {
          if (state.calendar_tombstones.length >= 4096) throw new Error('Cancellation ledger full');
          state.calendar_tombstones.push(found.id);
        }
      }
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
          remind_minutes: null, description: args.description || '', created_ms: Date.now(), all_day: span, series_uid: null, ends_at_ms: null});
      } else {
        const item = state.events.find(entry => entry.id === args.id);
        if (!item) throw new Error('Event not found');
        Object.assign(item, {title, starts_at_ms: Date.parse(start + 'T00:00:00Z'),
          remind_minutes: null, description: args.description || '', all_day: span, ends_at_ms: null});
        const series = state.calendar_series[item.series_uid];
        if (series) {
          if (!Array.isArray(series.modified_occurrence_ids)) series.modified_occurrence_ids = [];
          if (!series.modified_occurrence_ids.includes(item.id)) series.modified_occurrence_ids.push(item.id);
        }
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
        if (state.calendar_series[current.series_uid]) state.calendar_series[current.series_uid].title = title;
      } else {
        if (!state.calendar_tombstones.includes(current.series_uid)) {
          if (state.calendar_tombstones.length >= 4096) throw new Error('Cancellation ledger full');
          state.calendar_tombstones.push(current.series_uid);
        }
        state.events = state.events.filter(item => item.series_uid !== current.series_uid);
        delete state.calendar_series[current.series_uid];
      }
      break;
    }
    case 'clear_calendar_cancellations': state.calendar_tombstones = []; break;
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
    case 'set_appearance':
      if (!['comfortable', 'compact'].includes(args.density)) throw new Error('Invalid appearance density');
      if (typeof args.reduceMotion !== 'boolean' || typeof args.highContrast !== 'boolean') {
        throw new Error('Invalid accessibility preference');
      }
      Object.assign(state.settings, {
        ui_density: args.density, reduce_motion: args.reduceMotion,
        high_contrast: args.highContrast
      });
      break;
    case 'set_hover_to_open': state.settings.hover_to_open = args.enabled; break;
    case 'set_pin_open': state.settings.pin_open = args.enabled; break;
    case 'set_notch_on_top': state.settings.always_on_top = args.enabled; break;
    case 'set_fullscreen_auto': state.settings.auto_lower_fullscreen = Boolean(args.enabled); break;
    case 'set_notifications_paused': state.settings.notifications_paused = Boolean(args.paused); break;
    case 'set_task_due_reminders': state.settings.task_due_reminders_enabled = Boolean(args.enabled); break;
    case 'set_notification_preferences':
      state.settings.notifications_enabled = args.enabled;
      state.settings.notifications_show_details = Boolean(args.enabled && args.showDetails);
      break;
    case 'set_focus_duration': {
      if (![15, 25, 45, 60].includes(args.minutes)) throw new Error('Invalid focus preset');
      if (focus.phase !== 'focus') throw new Error('Set the focus duration after your break');
      focus.duration_seconds = args.minutes * 60;
      focus.remaining_seconds = focus.duration_seconds;
      focus.ends_at_ms = null;
      focus.completed = false; focus.paused = false; break;
    }
    case 'focus_toggle': {
      if (focus.ends_at_ms != null) { focus.ends_at_ms = null; focus.paused = true; }
      else {
        if (focus.remaining_seconds === 0) focus.remaining_seconds = focusPhaseSeconds(focus);
        focus.ends_at_ms = Date.now() + focus.remaining_seconds * 1000;
        focus.completed = false; focus.paused = false;
      }
      break;
    }
    case 'focus_reset': {
      focus.remaining_seconds = focusPhaseSeconds(focus);
      focus.ends_at_ms = null;
      focus.completed = false; focus.paused = false; break;
    }
    case 'set_focus_cycle': {
      if (focus.ends_at_ms != null || focus.paused) throw new Error('Reset the active timer before changing its mode');
      if (focus.cycle_enabled !== Boolean(args.enabled)) {
        Object.assign(focus, {cycle_enabled: Boolean(args.enabled), phase: 'focus', completed_in_cycle: 0,
          remaining_seconds: focus.duration_seconds, ends_at_ms: null, completed: false, paused: false});
      }
      break;
    }
    case 'advance_focus_cycle': break;
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
      $('#collapsed').setAttribute('aria-expanded', String(expanded));
      if (expanded) render();
    } catch (error) { showError(error); }
  });
  windowQueue = task;
  return task;
}

async function requestCollapse(restoreFocus = false) {
  if (typeof paletteIsOpen === 'function' && paletteIsOpen()) return;
  if (isExpanded && activeTab === 'notes' && !(await flushNote())) return;
  await setExpanded(false);
  if (restoreFocus && !isExpanded) $('#collapsed').focus({preventScroll: true});
}

async function switchTab(tab) {
  if (!$$('.tab').some(button => button.dataset.tab === tab)) return false;
  const request = ++tabRequest;
  if (tab === activeTab) return true;
  // Opening Notes must never silently commit a recovered draft; only leaving
  // the Notes workspace flushes the active editor first.
  if (activeTab === 'notes' && !(await flushNote())) return false;
  if (request !== tabRequest) return false;
  activeTab = tab;
  $$('.tab').forEach(button => {
    const selected = button.dataset.tab === tab;
    button.classList.toggle('active', selected);
  });
  $$('.panel').forEach(panel => panel.classList.toggle('active', panel.id === 'panel-' + tab));
  if (typeof wfsSyncNavigation === 'function') wfsSyncNavigation();
  if (tab === 'settings') void refreshBackups();
  if (tab === 'system') void refreshTelemetry();
  return true;
}

async function openModuleFromContent(tab) {
  if (await switchTab(tab)) {
    $$('.tab').find(button => button.dataset.tab === tab)?.focus();
  }
}

function node(tag, className, value) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (value != null) element.textContent = value;
  return element;
}

function parseTaskDue(raw) {
  if (!raw) return null;
  const ms = new Date(raw).getTime();
  if (!Number.isSafeInteger(ms) || ms < 946684800000 || ms > 4102444800000 ||
      asLocalInput(ms) !== raw) {
    throw new Error('Choose a valid local task due date between years 2000 and 2100.');
  }
  return ms;
}

function cancelTodoEdit() {
  selectedTodoId = null;
  $('#todo-edit-form').hidden = true;
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
    const priority = ['low', 'normal', 'high'].includes(item.priority) ? item.priority : 'normal';
    const due = item.due_at_ms;
    const meta = node('small', 'todo-metadata',
      priority.toUpperCase() + (due == null ? '' : ' · Due ' + new Date(due).toLocaleString()));
    if (due != null && due < Date.now() && !item.done) meta.classList.add('overdue');
    const edit = node('button', 'text-button todo-edit-button', 'Edit');
    edit.type = 'button';
    edit.setAttribute('aria-label', 'Edit task: ' + item.title);
    edit.addEventListener('click', () => {
      selectedTodoId = item.id;
      $('#todo-edit-title').value = item.title;
      $('#todo-edit-priority').value = priority;
      $('#todo-edit-due').value = due == null ? '' : asLocalInput(due);
      $('#todo-edit-form').hidden = false;
      $('#todo-edit-title').focus();
    });
    const remove = node('button', 'remove-task', '×');
    remove.type = 'button';
    remove.setAttribute('aria-label', `Delete task: ${item.title}`);
    remove.addEventListener('click', async () => {
      if (await mutation('remove_todo', { id: item.id }) && selectedTodoId === item.id) cancelTodoEdit();
    });
    line.append(check, label, meta, edit, remove);
    list.append(line);
  }
}

function renderNotes() {
  $('#note-count').textContent = String(data.notes.length);
  const list = $('#note-list');
  list.replaceChildren();
  const query = ($('#note-search')?.value || '').trim().toLocaleLowerCase();
  const filter = $('#note-tag-filter');
  const tags = [...new Set(data.notes.flatMap(note => Array.isArray(note.tags) ? note.tags : []))].sort();
  const selectedTag = filter.value;
  filter.replaceChildren(new Option('All tags', ''), ...tags.map(tag => new Option('#' + tag, tag)));
  filter.value = tags.includes(selectedTag) ? selectedTag : '';
  const sorted = [...data.notes]
    .filter(note => !filter.value || (Array.isArray(note.tags) && note.tags.includes(filter.value)))
    .filter(note => !query || (note.title + ' ' + note.body + ' ' +
      (Array.isArray(note.tags) ? note.tags.join(' ') : '')).toLocaleLowerCase().includes(query))
    .sort((left, right) => right.updated_ms - left.updated_ms);
  for (const note of sorted) {
    const button = node('button', 'note-card' + (selectedNoteId === note.id ? ' selected' : ''), note.title || 'Untitled');
    button.type = 'button';
    button.append(node('small', '', note.body.split('\n').find(text => text.trim()) || 'Empty note'));
    if (Array.isArray(note.tags) && note.tags.length) {
      const chips = node('small', 'note-tag-chips', note.tags.map(tag => '#' + tag).join('  '));
      chips.setAttribute('aria-hidden', 'true');
      button.append(chips);
    }
    button.addEventListener('click', async () => {
      if (!(await flushNote())) return;
      selectedNoteId = note.id;
      renderNotes();
    });
    list.append(button);
  }
  if (!sorted.length) list.append(node('div', 'empty', query ? 'No matching notes.' : 'Nothing saved yet.'));
  const current = data.notes.find(note => note.id === selectedNoteId);
  for (const control of [$('#note-title'), $('#note-body'), $('#note-tags'), $('#note-delete'), $('#note-save')]) {
    control.disabled = !current;
  }
  if (boundEditorId !== selectedNoteId) {
    $('#note-title').value = current?.title || '';
    $('#note-body').value = current?.body || '';
    $('#note-tags').value = Array.isArray(current?.tags) ? current.tags.join(', ') : '';
    boundEditorId = selectedNoteId;
  }
  const conflicted = Boolean(noteConflictDraft && noteConflictDraft.id === selectedNoteId);
  $('#note-conflict').hidden = !conflicted;
  $('#note-conflict-reload').disabled = !current;
  const message = conflicted ? 'Conflict — your draft is safe. Choose an action below.'
    : !current ? 'Choose or create a note'
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
  $('#event-end-time').hidden = enabled;
  $('#event-end-time-label').hidden = enabled;
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
  if (!event.all_day) return event.ends_at_ms
    ? eventDate(event.starts_at_ms) + ' – ' + eventDate(event.ends_at_ms)
    : eventDate(event.starts_at_ms);
  const end = new Date(Date.parse(event.all_day.end_date_exclusive + 'T00:00:00Z') - 86400000)
    .toISOString().slice(0, 10);
  return event.all_day.start_date === end ? end + ' · All day'
    : event.all_day.start_date + ' – ' + end + ' · All day';
}
function cancelEventEdit() {
  if (typeof closeSeriesShift === 'function') closeSeriesShift();
  selectedEventId = null;
  selectedSeriesEventId = null;
  $('#event-submit').textContent = 'Add event';
  $('#event-cancel').hidden = true;
  $('#event-title').value = '';
  $('#event-description').value = '';
  $('#event-end-time').value = '';
  $('#event-time').disabled = false;
  $('#event-all-day').disabled = false;
  setAllDayEditor(false);
  $('#event-start-date').value = '';
  $('#event-end-date').value = '';
}
function renderCalendar() {
  const events = [...data.events].sort((a, b) => a.starts_at_ms - b.starts_at_ms);
  $('#calendar-count').textContent = `${events.length} EVENT${events.length === 1 ? '' : 'S'}`;
  const localDay = asLocalInput(Date.now()).slice(0, 10);
  const upcoming = events.find(item => item.all_day
    ? item.all_day.end_date_exclusive > localDay
    : (item.ends_at_ms ?? item.starts_at_ms + 60000) >= Date.now() - 60000);
  $('#next-event').textContent = upcoming ? upcoming.title + ' · ' + eventDisplayDate(upcoming) : 'No upcoming events';
  const list = $('#event-list');
  list.replaceChildren();
  if (!events.length) {
    list.append(node('div', 'empty', 'Your calendar is clear. Create your first event.'));
    return;
  }
  for (const event of events) {
    const past = event.all_day ? event.all_day.end_date_exclusive <= localDay
      : (event.ends_at_ms ?? event.starts_at_ms) < Date.now();
    const line = node('div', 'event-item' + (past ? ' past' : ''));
    const date = node('time', 'event-date', eventDisplayDate(event));
    date.dateTime = event.all_day ? event.all_day.start_date : new Date(event.starts_at_ms).toISOString();
    const info = node('div', 'event-info');
    info.append(node('b', '', event.title));
    const reminderLabel = event.all_day ? 'All day · no midnight reminder'
      : event.remind_minutes == null ? 'Reminder off'
      : event.remind_minutes === 0 ? 'At event time'
      : String(event.remind_minutes) + ' min before';
    info.append(node('small', '', reminderLabel));
    if (event.series_uid) {
      const source = data.calendar_series?.[event.series_uid];
      const badge = source?.rrule
        ? 'Imported series · saved rule: ' + source.rrule
        : 'Imported series · legacy/unknown source rules';
      info.append(node('small', 'series-source-summary', badge));
    }
    const edit = node('button', 'text-button', 'Edit');
    edit.type = 'button';
    edit.setAttribute('aria-label', `Edit event: ${event.title}`);
    edit.addEventListener('click', () => {
      cancelEventEdit();
      selectedEventId = event.id;
      $('#event-title').value = event.title;
      $('#event-description').value = event.description || '';
      $('#event-time').value = asLocalInput(event.starts_at_ms);
      $('#event-end-time').value = event.ends_at_ms == null ? '' : asLocalInput(event.ends_at_ms);
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
        if (!window.confirm('Delete all occurrences of this imported series? WFS will remember the cancellation and skip it on future imports until you explicitly reset ICS cancellation history.')) return;
        if (await mutation('remove_event_series', {id: event.id})) cancelEventEdit();
      });
      const shiftSeries = node('button', 'text-button', 'Move series');
      shiftSeries.type = 'button';
      shiftSeries.setAttribute('aria-label', 'Preview whole-series day shift: ' + event.title);
      shiftSeries.addEventListener('click', () => {
        if (typeof openSeriesShift === 'function') openSeriesShift(event.id);
      });
      line.append(rename, shiftSeries, deleteSeries);
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
  const density = ['comfortable', 'compact'].includes(data.settings.ui_density)
    ? data.settings.ui_density : 'comfortable';
  document.documentElement.dataset.uiDensity = density;
  document.documentElement.dataset.uiMotion = data.settings.reduce_motion ? 'reduced' : 'system';
  document.documentElement.dataset.uiContrast = data.settings.high_contrast ? 'high' : 'standard';
  $('#ui-density').value = density;
  $('#ui-reduce-motion').checked = Boolean(data.settings.reduce_motion);
  $('#ui-high-contrast').checked = Boolean(data.settings.high_contrast);
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
  const taskAlerts = $('#task-reminders-toggle');
  taskAlerts.checked = Boolean(data.settings.task_due_reminders_enabled);
  taskAlerts.disabled = !nativeInvoke;
  $('#task-reminders-status').textContent = !nativeInvoke
    ? 'Native Windows only: browser preview never sends task toasts.'
    : !data.settings.notifications_enabled
      ? 'Enable WFS desktop reminders above before task alerts can be delivered.'
      : data.settings.notifications_paused
        ? 'WFS desktop reminders are paused; task alerts will not send.'
        : taskAlerts.checked
          ? 'Task reminders are on for unfinished tasks with a due date, only while WFS Notch is running.'
          : 'Task alerts are off. Due dates remain stored locally.';
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
  const phase = focus.phase === 'short_break' ? 'Short break' : focus.phase === 'long_break' ? 'Long break' : 'Focus';
  const action = phase === 'Focus' ? 'focus' : 'break';
  $('.timer-ring').style.setProperty('--timer-progress', `${100 * view.remaining / focusPhaseSeconds(focus)}%`);
  $('.timer-ring').dataset.phase = focus.phase;
  const transition = focus.cycle_enabled && view.completed;
  $('#timer-start').disabled = transition;
  $('#timer-reset').disabled = transition;
  $('#timer-start').textContent = transition ? 'Changing phase…' : view.running ? `Pause ${action}` : view.completed ? 'Start again' : focus.paused ? `Resume ${action}` : `Start ${action}`;
  $('#timer-label').textContent = transition ? 'CHANGING PHASE' : view.completed ? 'SESSION COMPLETE' : focus.paused ? 'PAUSED' : view.running ? phase.toUpperCase() : 'READY WHEN YOU ARE';
  $('#focus-cycle').checked = focus.cycle_enabled;
  $('#focus-cycle').disabled = focus.ends_at_ms != null || focus.paused;
  const status = focus.cycle_enabled ? `${phase} · ${focus.completed_in_cycle} / 4 focus sessions this cycle` : 'Single focus session';
  if ($('#focus-cycle-status').textContent !== status) $('#focus-cycle-status').textContent = status;
  $$('[data-minutes]').forEach(button => {
    const chosen = Number(button.dataset.minutes) * 60 === focus.duration_seconds;
    button.classList.toggle('selected', chosen);
    button.setAttribute('aria-pressed', String(chosen));
    button.disabled = focus.phase !== 'focus' || transition;
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

async function saveRevisionNote(draft) {
  // Match mutation() write ordering, but surface an optimistic-lock conflict
  // without discarding local editor input or silently retrying an overwrite.
  const task = writeQueue.catch(() => {}).then(async () => {
    try {
      const updated = await invoke('update_note', draft);
      data = updated;
      const committed = data.notes.find(note => note.id === draft.id);
      if (pendingDraft?.id === draft.id && committed) {
        // Rebase only a newer local keystroke after our own successful write.
        pendingDraft.expectedRevision = committed.revision || 0;
        persistPendingNote(pendingDraft);
      } else if (!pendingDraft) {
        persistPendingNote(null);
      }
      noteConflictDraft = null;
      render();
      return true;
    } catch (error) {
      if (!pendingDraft) pendingDraft = draft;
      persistPendingNote(pendingDraft);
      if (String(error).includes('NOTE_CONFLICT')) {
        noteConflictDraft = pendingDraft;
        renderNotes();
      } else {
        $('#save-status').textContent = 'Save failed — draft retained locally';
        showError(error);
      }
      return false;
    }
  });
  writeQueue = task;
  return task;
}

async function flushNote() {
  clearTimeout(noteSaveTimeout);
  noteSaveTimeout = null;
  if (noteConflictDraft) return false;
  if (!pendingDraft) return noteSaveInFlight ? noteSaveInFlight : true;
  const draft = pendingDraft;
  pendingDraft = null;
  $('#save-status').textContent = 'Saving…';
  const previous = noteSaveInFlight;
  const job = (async () => {
    if (previous) {
      if (!(await previous)) {
        if (!pendingDraft) pendingDraft = draft;
        persistPendingNote(pendingDraft);
        return false;
      }
      // Only a successful preceding save from this editor can advance its
      // revision. An out-of-process modification still fails in Rust.
      draft.expectedRevision = data.notes.find(note => note.id === draft.id)?.revision ??
        draft.expectedRevision;
    }
    return saveRevisionNote(draft);
  })();
  noteSaveInFlight = job;
  try { return await job; }
  finally { if (noteSaveInFlight === job) noteSaveInFlight = null; }
}

function scheduleNoteSave() {
  if (!selectedNoteId) return;
  const note = data.notes.find(item => item.id === selectedNoteId);
  pendingDraft = {
    id: selectedNoteId,
    title: $('#note-title').value,
    body: $('#note-body').value,
    tags: $('#note-tags').value.split(',').map(tag => tag.trim()).filter(Boolean),
    expectedRevision: note?.revision || 0,
  };
  if (noteConflictDraft) noteConflictDraft = pendingDraft;
  const persisted = persistPendingNote(pendingDraft);
  $('#save-status').textContent = noteConflictDraft ? 'Conflict — choose an action below'
    : persisted ? 'Unsaved changes' : 'Unsaved changes — local recovery unavailable';
  clearTimeout(noteSaveTimeout);
  if (!noteConflictDraft) noteSaveTimeout = setTimeout(() => { void flushNote(); }, 650);
}

function clockTick() {
  const now = new Date();
  const text = now.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  $('#closed-clock').textContent = text;
  $('#hero-time').textContent = text;
  $('#current-date').textContent = now.toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' });
  renderTimer();
  const view = focusView(data.focus, now.getTime());
  if (data.focus.cycle_enabled && data.focus.ends_at_ms != null && view.completed &&
      !focusAdvancePending && now.getTime() >= focusAdvanceRetryAt) {
    focusAdvancePending = true;
    void mutation('advance_focus_cycle').then(ok => {
      focusAdvanceRetryAt = Date.now() + (ok ? 500 : 5000);
      focusAdvancePending = false;
    });
  }
  if (!data.focus.cycle_enabled && view.completed && !focusCompletionNotified && !document.hidden) {
    focusCompletionNotified = true;
    showInfo('Focus session complete. Great work!');
  }
  if (!view.completed) focusCompletionNotified = false;
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
      event.preventDefault();
      void setExpanded(true).then(() => {
        if (isExpanded && !$('#command-palette').hidden) return;
        if (isExpanded) $('.tab.active')?.focus();
      });
    }
  });
  $('#notch').addEventListener('mouseleave', () => {
    if (nativeInvoke && data.settings.hover_to_open && !data.settings.pin_open) void requestCollapse();
  });
  $('#collapse').addEventListener('click', () => void requestCollapse(true));
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape' && isExpanded && !event.defaultPrevented && $('#command-palette').hidden) {
      event.preventDefault();
      void requestCollapse(true);
    }
  });
  $$('.tab').forEach(button => button.addEventListener('click', async () => {
    if (!(await switchTab(button.dataset.tab)) && document.activeElement === button) {
      $('.tab.active')?.focus();
    }
  }));
  $('#go-timer').addEventListener('click', () => void openModuleFromContent('timer'));
  $('#go-calendar').addEventListener('click', () => void openModuleFromContent('calendar'));
  $('#event-all-day').addEventListener('change', () => setAllDayEditor($('#event-all-day').checked));
  $('#event-form').addEventListener('submit', async event => {
    event.preventDefault();
    const title = $('#event-title').value.trim();
    if (!title) { showError('Enter an event title'); return; }
    if (selectedSeriesEventId) {
      if (await mutation('update_event_series_title', {id: selectedSeriesEventId, title})) {
        cancelEventEdit();
        showInfo('All imported series occurrence titles updated.');
      }
      return;
    }
    const description = $('#event-description').value;
    if ($('#event-all-day').checked) {
      const startDate = $('#event-start-date').value;
      const endDateExclusive = $('#event-end-date').value;
      const mode = selectedEventId ? 'update_all_day_event' : 'add_all_day_event';
      if (await mutation(mode, {
        ...(selectedEventId ? {id: selectedEventId} : {}),
        title, startDate, endDateExclusive, description
      })) {
        cancelEventEdit();
        showInfo('All-day calendar dates saved (exclusive end; no reminder).');
      }
      return;
    }
    const startsAtMs = new Date($('#event-time').value).getTime();
    const endRaw = $('#event-end-time').value;
    const endsAtMs = endRaw ? new Date(endRaw).getTime() : null;
    if (!Number.isFinite(startsAtMs) || !validTimedEnd(startsAtMs, endsAtMs)) {
      showError('Enter valid timed start/end dates. End must be later and at most 366 days after start.'); return;
    }
    const remind = $('#event-remind').value;
    const mode = selectedEventId ? 'update_event' : 'add_event';
    if (await mutation(mode, {
      ...(selectedEventId ? { id: selectedEventId } : {}),
      title, startsAtMs, endsAtMs, description,
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
    let dueAtMs;
    try { dueAtMs = parseTaskDue($('#todo-due').value); }
    catch (error) { showError(error); return; }
    if (title && await mutation('add_todo', {
      title, priority: $('#todo-priority').value, dueAtMs,
    })) {
      input.value = '';
      $('#todo-priority').value = 'normal';
      $('#todo-due').value = '';
    }
  });
  $('#todo-edit-form').addEventListener('submit', async event => {
    event.preventDefault();
    if (!selectedTodoId) return;
    let dueAtMs;
    try { dueAtMs = parseTaskDue($('#todo-edit-due').value); }
    catch (error) { showError(error); return; }
    if (await mutation('update_todo', {
      id: selectedTodoId, title: $('#todo-edit-title').value,
      priority: $('#todo-edit-priority').value, dueAtMs,
    })) cancelTodoEdit();
  });
  $('#todo-edit-cancel').addEventListener('click', cancelTodoEdit);

  // Explicit local-only JSON transfer. Preview never writes and each import
  // rechecks the full payload and existing IDs under Rust's storage transaction.
  $('#notes-import-json').addEventListener('input', () => {
    notesImportPreview = null;
    notesImportPayload = null;
    $('#notes-import-apply').disabled = true;
    $('#notes-transfer-status').textContent = 'Changed input. Preview again before importing.';
  });
  $('#notes-export-button').addEventListener('click', async () => {
    if (!(await flushNote())) return;
    await writeQueue;
    try {
      const payload = await invoke('export_notes_json');
      $('#notes-export-json').value = payload;
      $('#notes-export-json').focus();
      $('#notes-export-json').select();
      $('#notes-transfer-status').textContent = 'JSON prepared locally. Copy the selected text to a safe private file.';
    } catch (error) {
      $('#notes-transfer-status').textContent = 'Cannot export Notes JSON.';
      showError(error);
    }
  });
  $('#notes-import-preview').addEventListener('click', async () => {
    const payload = $('#notes-import-json').value;
    notesImportPayload = null;
    notesImportPreview = null;
    $('#notes-import-apply').disabled = true;
    try {
      if (!(await flushNote())) return;
      await writeQueue;
      notesImportPreview = await invoke('preview_notes_import', {payload});
      notesImportPayload = payload;
      const result = notesImportPreview;
      $('#notes-transfer-status').textContent = result.new_count + ' new note(s), ' +
        result.existing_count + ' existing ID(s) will be skipped. Nothing has been imported.';
      $('#notes-import-apply').disabled = result.new_count === 0;
    } catch (error) {
      $('#notes-transfer-status').textContent = 'Import preview rejected: ' + (error?.message || String(error));
    }
  });
  $('#notes-import-apply').addEventListener('click', async () => {
    if (!notesImportPreview || notesImportPayload !== $('#notes-import-json').value ||
      notesImportPreview.new_count === 0) return;
    if (!(await flushNote())) return;
    if (!window.confirm('Import new Notes from the previewed JSON? Existing note IDs and content will never be replaced.')) return;
    const before = new Set(data.notes.map(note => note.id));
    if (await mutation('import_notes_json', {payload:notesImportPayload})) {
      const imported = data.notes.filter(note => !before.has(note.id)).length;
      $('#notes-transfer-status').textContent = 'Imported ' + imported +
        ' new note(s). Existing Notes were preserved; repeat imports skip matching IDs.';
      notesImportPayload = null;
      notesImportPreview = null;
      $('#notes-import-apply').disabled = true;
    }
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
  $('#note-tags').addEventListener('input', scheduleNoteSave);
  $('#note-title').addEventListener('blur', () => { void flushNote(); });
  $('#note-body').addEventListener('blur', () => { void flushNote(); });
  $('#note-tags').addEventListener('blur', () => { void flushNote(); });
  $('#note-tag-filter').addEventListener('change', renderNotes);
  $('#note-save').addEventListener('click', () => { void flushNote(); });
  $('#note-conflict-reload').addEventListener('click', async () => {
    if (!noteConflictDraft || !window.confirm('Discard the unsaved draft and load the latest stored note?')) return;
    await writeQueue;
    try {
      data = await invoke('load_data');
      const latest = data.notes.find(note => note.id === selectedNoteId);
      if (!latest) { showError('The original note no longer exists. Save the draft as a new note.'); return; }
      noteConflictDraft = null;
      pendingDraft = null;
      persistPendingNote(null);
      boundEditorId = null;
      render();
    } catch (error) { showError(error); }
  });
  $('#note-conflict-copy').addEventListener('click', async () => {
    if (!noteConflictDraft) return;
    const draft = pendingDraft || noteConflictDraft;
    const before = new Set(data.notes.map(note => note.id));
    const copied = await mutation('create_note_from_draft', {
      title:draft.title, body:draft.body, tags:draft.tags
    });
    if (!copied) return;
    noteConflictDraft = null;
    pendingDraft = null;
    persistPendingNote(null);
    selectedNoteId = data.notes.find(note => !before.has(note.id))?.id || null;
    boundEditorId = null;
    renderNotes();
    showInfo('Saved a separate note; the newer original was not changed.');
  });
  $('#note-delete').addEventListener('click', async () => {
    if (!selectedNoteId || noteConflictDraft || !(await flushNote())) return;
    const id = selectedNoteId;
    const current = data.notes.find(note => note.id === id);
    if (!current) return;
    if (await mutation('remove_note', { id, expectedRevision:current.revision || 0 })) {
      selectedNoteId = null;
      boundEditorId = null;
      pendingDraft = null;
      persistPendingNote(null);
      renderNotes();
    }
  });
  $$('.swatch').forEach(button => button.addEventListener('click', () => void mutation('set_accent', { accent: button.dataset.color })));
  const saveAppearance = () => {
    void mutation('set_appearance', {
      density: $('#ui-density').value,
      reduceMotion: $('#ui-reduce-motion').checked,
      highContrast: $('#ui-high-contrast').checked,
    });
  };
  for (const control of [$('#ui-density'), $('#ui-reduce-motion'), $('#ui-high-contrast')]) {
    control.addEventListener('change', saveAppearance);
  }
  $('#ui-reset').addEventListener('click', () => {
    void mutation('set_appearance', {
      density: 'comfortable', reduceMotion: false, highContrast: false,
    });
  });
  $('#hover-toggle').addEventListener('change', event => void mutation('set_hover_to_open', { enabled: event.target.checked }));
  $('#pin-toggle').addEventListener('change', event => void mutation('set_pin_open', { enabled: event.target.checked }));
  $('#on-top-toggle').addEventListener('change', event => void mutation('set_notch_on_top', { enabled: event.target.checked }));
  $('#fullscreen-auto-toggle').addEventListener('change', event => void mutation('set_fullscreen_auto', { enabled: event.target.checked }));
  $('#notifications-toggle').addEventListener('change', event => void mutation('set_notification_preferences', { enabled: event.target.checked, showDetails: event.target.checked && $('#notification-details-toggle').checked }));
  $('#notification-details-toggle').addEventListener('change', event => void mutation('set_notification_preferences', { enabled: $('#notifications-toggle').checked, showDetails: event.target.checked }));
  $('#notifications-pause-toggle').addEventListener('change', event => void mutation('set_notifications_paused', { paused: event.target.checked }));
  $('#task-reminders-toggle').addEventListener('change', event =>
    void mutation('set_task_due_reminders', { enabled: event.target.checked }));
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
      const retrying = Math.max(0, Number(status.pending_retry_count || 0));
      const exhausted = Math.max(0, Number(status.suppressed_after_failures_count || 0));
      const retryNote = ' Sender retries: ' + retrying + ' pending, ' + exhausted +
        ' exhausted (up to ' + (status.max_sender_attempts || 3) +
        ' enqueue attempts; Windows may still hide accepted toasts).';
      $('#notification-scheduler-status').textContent =
        'Native reminder scheduler: ' + state + '. ' + count +
        ' upcoming calendar reminder(s); checks every ' + status.poll_interval_seconds +
        ' seconds while WFS Notch is running. ' +
        Math.max(0, Number(status.upcoming_task_due_count || 0)) +
        ' upcoming opted-in task due reminder(s).' + next + retryNote;
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
  $('#focus-cycle').addEventListener('change', async event => {
    const control = event.currentTarget;
    control.disabled = true;
    if (!(await mutation('set_focus_cycle', {enabled: control.checked}))) renderTimer();
  });
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
  const recoveredDraft = readPendingNote();
  if (recoveredDraft) {
    selectedNoteId = recoveredDraft.id;
    pendingDraft = recoveredDraft;
    boundEditorId = recoveredDraft.id;
    $('#note-title').value = recoveredDraft.title;
    $('#note-body').value = recoveredDraft.body;
    $('#note-tags').value = recoveredDraft.tags.join(', ');
    const current = data.notes.find(note => note.id === recoveredDraft.id);
    if (!current || (current.revision || 0) !== recoveredDraft.expectedRevision) {
      noteConflictDraft = recoveredDraft;
    }
  }
  if (!nativeInvoke || data.settings.pin_open) await setExpanded(true);
  render();
  if (recoveredDraft) showInfo('An unsaved local note draft was recovered. Open Notes to review it.');
  void refreshBackups();
  clockTick();
  setInterval(clockTick, 250);
}

void start();
