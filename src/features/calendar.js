'use strict';
// Local calendar month navigator + native RFC 5545 text interchange.
// The native ICS parser is implemented in Rust; no remote calendar service is contacted.
let calendarPlusInitialized = false;
let calendarMonth = new Date(new Date().getFullYear(), new Date().getMonth(), 1);
let calendarSelectedDay = null;
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
      const key = calendarLocalKey(new Date(event.starts_at_ms));
      countByDay.set(key, (countByDay.get(key) || 0) + 1);
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
  const status = document.querySelector('#ics-status');
  if (!nativeInvoke) {
    status.textContent = 'ICS import/export requires the native Windows Rust app. Monthly calendar and note search work in browser preview.';
  }
  document.querySelector('#ics-import-button').addEventListener('click', async () => {
    const text = document.querySelector('#ics-import-data').value.trim();
    if (!nativeInvoke) { showError('ICS import requires the installed WFS Notch desktop app.'); return; }
    if (!text || text.length > 262144) { showError('Paste a valid ICS calendar (up to 256 KiB).'); return; }
    const button = document.querySelector('#ics-import-button');
    button.disabled = true;
    try {
      if (!(await flushNote())) return;
      await writeQueue;
      const before = data.events.length;
      const floatingTimezone = document.querySelector('#ics-floating-timezone').value.trim();
      data = await invoke('import_ics', {icsText: text, floatingTimezone: floatingTimezone || null});
      render();
      const imported = data.events.length - before;
      status.textContent = imported + ' new event(s) imported (recurrence flattened to UTC). Existing UIDs skipped.';
      showInfo(status.textContent);
    } catch (error) {
      status.textContent = 'Import not applied. Check the IANA timezone, DST ambiguity, rule support and event size.';
      showError(error);
    } finally {
      button.disabled = false;
    }
  });
  document.querySelector('#ics-export-button').addEventListener('click', async () => {
    if (!nativeInvoke) { showError('ICS export requires the installed WFS Notch desktop app.'); return; }
    try {
      await writeQueue;
      const text = await invoke('export_ics');
      document.querySelector('#ics-export-data').value = text;
      status.textContent = 'Export prepared (' + data.events.length + ' events). Copy and save as .ics in a UTF-8 text editor.';
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
