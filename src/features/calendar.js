'use strict';
// Local calendar month navigator + native RFC 5545 text interchange.
// The native ICS parser is implemented in Rust; no remote calendar service is contacted.
let calendarPlusInitialized = false;
let calendarMonth = new Date(new Date().getFullYear(), new Date().getMonth(), 1);
let calendarSelectedDay = null;
let calendarShiftEventId = null;
let calendarShiftPlan = null;

function closeSeriesShift() {
  calendarShiftEventId = null;
  calendarShiftPlan = null;
  const panel = document.querySelector('#calendar-series-shift');
  if (panel) panel.hidden = true;
  const apply = document.querySelector('#calendar-shift-apply');
  if (apply) apply.disabled = true;
}

function openSeriesShift(id) {
  cancelEventEdit();
  calendarShiftEventId = id;
  calendarShiftPlan = null;
  const panel = document.querySelector('#calendar-series-shift');
  panel.hidden = false;
  document.querySelector('#calendar-shift-days').value = '1';
  document.querySelector('#calendar-shift-apply').disabled = true;
  document.querySelector('#calendar-shift-status').textContent = nativeInvoke
    ? 'Preview this local series schedule before confirming. Only untouched UTC or all-day series are supported.'
    : 'Series schedule shifts require the installed Windows desktop app; preview mode never fakes a source recurrence engine.';
  document.querySelector('#calendar-shift-days').focus();
}

function seriesShiftDays() {
  const value = Number(document.querySelector('#calendar-shift-days').value);
  if (!Number.isInteger(value) || value === 0 || value < -365 || value > 365) {
    throw new Error('Choose an integer number of days from −365 to −1 or 1 to 365.');
  }
  return value;
}

function seriesShiftSummary(plan) {
  const utcDate = ms => new Date(ms).toISOString().slice(0, 10);
  return plan.occurrence_count + ' saved occurrence(s): first ' +
    utcDate(plan.first_old_ms) + ' → ' + utcDate(plan.first_new_ms) +
    ', last ' + utcDate(plan.last_old_ms) + ' → ' + utcDate(plan.last_new_ms) +
    '. ' + plan.reminders_affected + ' reminder(s), ' +
    plan.same_start_conflicts + ' exact-start collision(s) with other events. ' +
    'Existing deletions are not regenerated. Export will flatten shifted occurrences.';
}

function calendarLocalKey(day) {
  const pad = n => String(n).padStart(2, '0');
  return [day.getFullYear(), pad(day.getMonth() + 1), pad(day.getDate())].join('-');
}
function renderCalendarPlus() {
  const container = document.querySelector('#calendar-month-grid');
  if (!container) return;
  const year = calendarMonth.getFullYear();
  const month = calendarMonth.getMonth();
  const first = new Date(year, month, 1);
  const lastDay = new Date(year, month + 1, 0).getDate();
  const padding = (first.getDay() + 6) % 7;
  const totalCells = Math.ceil((padding + lastDay) / 7) * 7;
  const countByDay = new Map();
  for (const event of data.events || []) {
    if (event.all_day) {
      // DATE spans are civil dates, not local or UTC instant conversions.
      // End date is exclusive; each covered date receives one calendar badge.
      for (let day = 1; day <= lastDay; day++) {
        const key = calendarLocalKey(new Date(year, month, day, 12));
        if (key >= event.all_day.start_date && key < event.all_day.end_date_exclusive) {
          countByDay.set(key, (countByDay.get(key) || 0) + 1);
        }
      }
    } else {
      // For exclusive timed ranges, count every local civil day intersecting
      // [start,end). Local midnight boundaries handle DST 23/25-hour days.
      const end = event.ends_at_ms ?? (event.starts_at_ms + 1);
      for (let day = 1; day <= lastDay; day++) {
        const begins = new Date(year, month, day).getTime();
        const nextDay = new Date(year, month, day + 1).getTime();
        if (event.starts_at_ms < nextDay && end > begins) {
          const key = calendarLocalKey(new Date(year, month, day, 12));
          countByDay.set(key, (countByDay.get(key) || 0) + 1);
        }
      }
    }
  }
  document.querySelector('#calendar-month-title').textContent =
    first.toLocaleDateString(undefined, {month: 'long', year: 'numeric'});
  container.replaceChildren();
  const today = calendarLocalKey(new Date());
  for (let index = 0; index < totalCells; index++) {
    const date = index - padding + 1;
    if (date < 1 || date > lastDay) {
      const empty = document.createElement('span');
      empty.className = 'calendar-blank';
      empty.setAttribute('aria-hidden', 'true');
      container.append(empty);
      continue;
    }
    const day = new Date(year, month, date, 12);
    const key = calendarLocalKey(day);
    const n = countByDay.get(key) || 0;
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'calendar-day';
    button.classList.toggle('today', key === today);
    button.classList.toggle('selected', key === calendarSelectedDay);
    button.classList.toggle('has-event', n > 0);
    button.setAttribute('aria-label', day.toLocaleDateString(undefined, {day: 'numeric', month: 'long', year: 'numeric'}) + ': ' + n + ' event(s)');
    button.setAttribute('aria-pressed', String(key === calendarSelectedDay));
    const number = document.createElement('span');
    number.textContent = String(date);
    button.append(number);
    if (n) {
      const mark = document.createElement('small');
      mark.textContent = n < 10 ? String(n) : '9+';
      mark.setAttribute('aria-hidden', 'true');
      button.append(mark);
    }
    button.addEventListener('click', () => {
      calendarSelectedDay = key;
      const field = document.querySelector('#event-time');
      const previousTime = field.value.includes('T') ? field.value.split('T')[1] : '09:00';
      if (document.querySelector('#event-all-day').checked) {
        document.querySelector('#event-start-date').value = key;
        document.querySelector('#event-end-date').value = calendarLocalKey(new Date(year, month, date + 1, 12));
        document.querySelector('#event-start-date').focus();
      } else {
        field.value = key + 'T' + previousTime;
        field.focus();
      }
      renderCalendarPlus();
    });
    container.append(button);
  }
}
function initCalendarPlus() {
  if (calendarPlusInitialized) return;
  calendarPlusInitialized = true;
  document.querySelector('#calendar-prev').addEventListener('click', () => {
    calendarMonth = new Date(calendarMonth.getFullYear(), calendarMonth.getMonth() - 1, 1);
    renderCalendarPlus();
  });
  document.querySelector('#calendar-next').addEventListener('click', () => {
    calendarMonth = new Date(calendarMonth.getFullYear(), calendarMonth.getMonth() + 1, 1);
    renderCalendarPlus();
  });
  document.querySelector('#calendar-today').addEventListener('click', () => {
    calendarMonth = new Date(new Date().getFullYear(), new Date().getMonth(), 1);
    calendarSelectedDay = calendarLocalKey(new Date());
    renderCalendarPlus();
  });
  document.querySelector('#note-search').addEventListener('input', () => renderNotes());
  document.querySelector('#calendar-shift-days').addEventListener('input', () => {
    calendarShiftPlan = null;
    document.querySelector('#calendar-shift-apply').disabled = true;
    document.querySelector('#calendar-shift-status').textContent =
      'Day offset changed. Preview the updated schedule again.';
  });
  document.querySelector('#calendar-shift-cancel').addEventListener('click', closeSeriesShift);
  document.querySelector('#calendar-shift-preview').addEventListener('click', async () => {
    const statusNode = document.querySelector('#calendar-shift-status');
    if (!nativeInvoke) {
      statusNode.textContent = 'Requires native Windows. Browser preview does not fake recurrence scheduling.';
      return;
    }
    let deltaDays;
    try { deltaDays = seriesShiftDays(); }
    catch (error) { showError(error); return; }
    const id = calendarShiftEventId;
    if (!id) return;
    const button = document.querySelector('#calendar-shift-preview');
    button.disabled = true;
    try {
      if (!(await flushNote())) return;
      await writeQueue;
      const plan = await invoke('preview_event_series_shift', {id, deltaDays});
      if (calendarShiftEventId !== id ||
          Number(document.querySelector('#calendar-shift-days').value) !== deltaDays) return;
      if (!plan || typeof plan.fingerprint !== 'string' || !/^[0-9a-f]{16}$/.test(plan.fingerprint)) {
        throw new Error('Preview returned an invalid freshness token');
      }
      calendarShiftPlan = plan;
      document.querySelector('#calendar-shift-apply').disabled = false;
      statusNode.textContent = seriesShiftSummary(plan);
    } catch (error) {
      calendarShiftPlan = null;
      document.querySelector('#calendar-shift-apply').disabled = true;
      statusNode.textContent = 'This series cannot be safely moved. No calendar data was changed.';
      showError(error);
    } finally { button.disabled = false; }
  });
  document.querySelector('#calendar-shift-apply').addEventListener('click', async () => {
    const plan = calendarShiftPlan, id = calendarShiftEventId;
    if (!nativeInvoke || !plan || !id) return;
    let days;
    try { days = seriesShiftDays(); }
    catch (error) { showError(error); return; }
    if (plan.delta_days !== days) {
      calendarShiftPlan = null;
      document.querySelector('#calendar-shift-apply').disabled = true;
      showError('The day offset changed. Generate a new preview.'); return;
    }
    if (!window.confirm('Move ' + plan.occurrence_count + ' existing local occurrences by ' +
      days + ' day(s)? The original RRULE will no longer describe the shifted schedule; ' +
      'ICS export will use independent events. Existing canceled occurrences stay canceled.')) return;
    const button = document.querySelector('#calendar-shift-apply');
    button.disabled = true;
    if (await mutation('shift_event_series', {
      id, deltaDays: days, expectedFingerprint: plan.fingerprint
    })) {
      closeSeriesShift();
      showInfo('Existing local series occurrences moved. The original recurrence source remains unchanged.');
    } else {
      calendarShiftPlan = null;
      document.querySelector('#calendar-shift-status').textContent =
        'Move was not applied. Preview the current series schedule again.';
    }
  });
  const status = document.querySelector('#ics-status');
  document.querySelector('#ics-clear-cancellations').addEventListener('click', async () => {
    const count = Array.isArray(data.calendar_tombstones) ? data.calendar_tombstones.length : 0;
    if (!count) { showInfo('No locally canceled ICS events to reset.'); return; }
    if (!window.confirm('Allow previously deleted imported ICS events and series to appear on your NEXT explicit import? This only clears local cancellation markers.')) return;
    if (await mutation('clear_calendar_cancellations')) {
      status.textContent = 'ICS cancellations reset. Previously deleted items will only return after you explicitly reimport their calendar.';
      showInfo(status.textContent);
    }
  });
  if (!nativeInvoke) {
    status.textContent = 'ICS import/export requires the native Windows Rust app. Monthly calendar and note search work in browser preview.';
  }
  const input = $('#ics-import-data');
  const timezoneInput = $('#ics-floating-timezone');
  const previewButton = $('#ics-import-preview');
  const applyButton = $('#ics-import-button');
  const cancellationList = $('#ics-cancellation-preview');
  let importPlan = null;
  let inputRevision = 0;
  let importing = false;
  const readImport = () => ({icsText: input.value.trim(), floatingTimezone: timezoneInput.value.trim() || null});
  function invalidateImport() {
    inputRevision += 1;
    importPlan = null;
    applyButton.disabled = true;
    cancellationList.replaceChildren();
    cancellationList.hidden = true;
  }
  for (const control of [input, timezoneInput]) {
    control.addEventListener('input', () => {
      invalidateImport();
      if (nativeInvoke) status.textContent = 'Input changed. Preview the import before applying.';
    });
  }
  previewButton.addEventListener('click', async () => {
    if (!nativeInvoke) { showError('ICS import requires the installed WFS Notch desktop app.'); return; }
    if (importing) return;
    invalidateImport();
    const revision = inputRevision;
    const payload = readImport();
    if (!payload.icsText || new TextEncoder().encode(payload.icsText).length > 262144) {
      status.textContent = 'Paste a valid ICS calendar up to 256 KiB.';
      return;
    }
    previewButton.disabled = true;
    status.textContent = 'Checking calendar changes…';
    try {
      await writeQueue;
      const plan = await invoke('preview_ics_import', payload);
      if (revision !== inputRevision) return;
      importPlan = {payload, plan};
      for (const entry of plan.cancellations) {
        const timezone = entry.original_kind.includes('VALUE=DATE') || /^\d{8}$/.test(entry.original)
          ? 'all day' : entry.original.endsWith('Z') ? 'UTC'
            : entry.original_kind.split('TZID=')[1] || entry.floating_timezone || 'floating';
        const value = entry.original;
        const date = `${value.slice(0,4)}-${value.slice(4,6)}-${value.slice(6,8)}`;
        const time = value.includes('T') ? ` ${value.slice(9,11)}:${value.slice(11,13)}:${value.slice(13,15)}` : '';
        cancellationList.append(node('li', '', `${entry.title} · ${date}${time} (${timezone}) · ${entry.removes_local_event ? 'Remove saved occurrence' : 'Keep occurrence excluded'}`));
      }
      cancellationList.hidden = plan.cancellations.length === 0;
      status.textContent = `Preview only. ${plan.new_events} new event(s), ${plan.removed_events} saved occurrence(s) removed, ${plan.cancellations.length} cancellation marker(s).`;
      applyButton.disabled = false;
    } catch (error) {
      if (revision === inputRevision) {
        status.textContent = 'Could not preview the import. No calendar changes were applied.';
        showError(error);
      }
    } finally { previewButton.disabled = false; }
  });
  applyButton.addEventListener('click', async () => {
    if (!importPlan || importing) return;
    const {payload, plan} = importPlan;
    const current = readImport();
    if (current.icsText !== payload.icsText || current.floatingTimezone !== payload.floatingTimezone) {
      invalidateImport();
      return;
    }
    importing = true;
    input.disabled = true;
    timezoneInput.disabled = true;
    previewButton.disabled = true;
    invalidateImport();
    status.textContent = 'Applying the reviewed import…';
    try {
      const applied = await mutation('import_ics', {...payload, expectedFingerprint: plan.fingerprint});
      status.textContent = applied
        ? `Imported ${plan.new_events} new event(s), removed ${plan.removed_events} saved occurrence(s), recorded ${plan.cancellations.length} cancellation marker(s).`
        : 'Import not applied. Preview the current changes again.';
      if (applied) showInfo(status.textContent);
    } finally {
      importing = false;
      input.disabled = false;
      timezoneInput.disabled = false;
      previewButton.disabled = false;
    }
  });
  document.querySelector('#ics-export-button').addEventListener('click', async () => {
    if (!nativeInvoke) { showError('ICS export requires the installed WFS Notch desktop app.'); return; }
    try {
      await writeQueue;
      const text = await invoke('export_ics');
      document.querySelector('#ics-export-data').value = text;
      status.textContent = 'Export prepared (' + data.events.length + ' local events). Unchanged ICS series retain their master and supported source exceptions; locally edited/deleted or legacy series may export as individual events. Copy and save as .ics.';
      showInfo('ICS export ready.');
    } catch (error) { showError(error); }
  });
  document.querySelector('#ics-export-copy').addEventListener('click', async () => {
    const text = document.querySelector('#ics-export-data').value;
    if (!text) { showError('Export events first.'); return; }
    try { await navigator.clipboard.writeText(text); showInfo('ICS copied to clipboard.'); }
    catch { showError('Clipboard permission unavailable. Select and copy the ICS text manually.'); }
  });
  renderCalendarPlus();
}
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', initCalendarPlus, {once: true});
else initCalendarPlus();
