'use strict';
// Additional independent original modules, based on observed reference behavior.
let wellnessInitialized = false;
let countdownSeenComplete = false;
let flashcardNoteId = '';
let flashcardIndex = 0;
let flashcardShowingAnswer = false;
let flashcardSignature = '';
let weatherInFlight = false;
let lastDayRefresh = 0;

function todayHealth() {
  const day = localDay();
  const source = data.health || {};
  return source.day === day
    ? source
    : {day, water_glasses: 0, break_count: 0, calm_minutes: 0};
}
function countdownView() {
  const timer = data.countdown || defaults().countdown;
  const remaining = timer.ends_at_ms == null
    ? timer.remaining_seconds
    : Math.max(0, Math.ceil((timer.ends_at_ms - Date.now()) / 1000));
  return {remaining, running: timer.ends_at_ms != null && remaining > 0, completed: remaining === 0};
}
function renderHealth() {
  const h = todayHealth();
  $('#health-day').textContent = h.day;
  $('#health-water').textContent = h.water_glasses + ' / 8';
  $('#health-water-meter').style.width = Math.min(100, h.water_glasses / 8 * 100) + '%';
  $('#health-breaks').textContent = String(h.break_count);
  $('#health-calm').textContent = String(h.calm_minutes);
}
function renderCountdown() {
  const timer = countdownView();
  const hours = Math.floor(timer.remaining / 3600);
  const mins = Math.floor(timer.remaining % 3600 / 60);
  const secs = timer.remaining % 60;
  $('#countdown-display').textContent =
    (hours ? String(hours).padStart(2,'0') + ':' : '') +
    String(mins).padStart(2,'0') + ':' + String(secs).padStart(2,'0');
  $('#countdown-toggle').textContent = timer.running ? 'Pause' : timer.completed ? 'Restart' : 'Start / resume';
  $('#countdown-status').textContent = timer.running ? 'Countdown running.' :
    timer.completed ? 'Countdown complete.' : 'Paused or ready.';
  if (timer.completed && !countdownSeenComplete && (data.countdown?.completed || data.countdown?.ends_at_ms != null) && !document.hidden) {
    countdownSeenComplete = true;
    showInfo('Countdown complete.');
  }
  if (!timer.completed) countdownSeenComplete = false;
}
function renderDay() {
  const now = new Date();
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const end = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1).getTime();
  const ratio = Math.max(0, Math.min(1, (Date.now() - start) / (end - start)));
  $('#day-percent').textContent = Math.floor(ratio * 100) + '%';
  $('#day-clock').textContent = now.toLocaleTimeString(undefined, {hour:'2-digit',minute:'2-digit'});
  $('#day-progress-meter').style.width = (ratio * 100).toFixed(2) + '%';
  const finished = data.todos.filter(t => t.done).length;
  $('#day-task-progress').textContent = finished + ' / ' + data.todos.length + ' tasks completed';
  const events = data.events.filter(e => e.starts_at_ms >= start && e.starts_at_ms < end)
    .sort((a,b) => a.starts_at_ms - b.starts_at_ms);
  $('#day-events-count').textContent = events.length + ' events today';
  const list = $('#day-event-list');
  list.replaceChildren();
  if (!events.length) {
    list.append(node('div','empty','No events scheduled for today.'));
  } else {
    for (const event of events) {
      list.append(node('div','module-item',new Date(event.starts_at_ms).toLocaleTimeString(undefined, {hour:'2-digit',minute:'2-digit'}) + ' · ' + event.title));
    }
  }
}
function cardsFromNote(id) {
  const note = data.notes.find(n => n.id === id);
  if (!note) return [];
  return note.body.split(/\r?\n/).slice(0,1000).flatMap(line => {
    const pos = line.indexOf('::');
    if (pos < 1) return [];
    const question = line.slice(0,pos).trim();
    const answer = line.slice(pos+2).trim();
    return question && answer ? [{question,answer}] : [];
  }).slice(0,200);
}
function renderFlashcards() {
  const signature = JSON.stringify(data.notes.map(n => [n.id,n.title,n.updated_ms]));
  const select = $('#flashcards-note');
  if (signature !== flashcardSignature) {
    flashcardSignature = signature;
    select.replaceChildren();
    for (const note of data.notes) {
      const option = document.createElement('option');
      option.value = note.id; option.textContent = note.title || 'Untitled';
      select.append(option);
    }
    if (!data.notes.some(n=>n.id===flashcardNoteId)) flashcardNoteId = data.notes[0]?.id || '';
    select.value = flashcardNoteId;
  }
  const cards = cardsFromNote(flashcardNoteId);
  if (flashcardIndex >= cards.length) flashcardIndex = 0;
  $('#flashcards-count').textContent = cards.length + ' CARDS';
  $('#flashcard-face').textContent = cards.length ?
    (flashcardShowingAnswer ? cards[flashcardIndex].answer : cards[flashcardIndex].question) :
    'Create a note with lines like “What is Rust? :: A programming language”.';
  $('#flashcard-flip').disabled = !cards.length;
  $('#flashcard-next').disabled = !cards.length;
  $('#flashcard-flip').textContent = flashcardShowingAnswer ? 'Show question' : 'Show answer';
  $('#flashcard-position').textContent = cards.length ?
    'Card ' + (flashcardIndex + 1) + ' of ' + cards.length : 'No cards in this note';
}
function renderParity() {
  if (!$('#health-water')) return;
  renderHealth(); renderDay(); renderCountdown(); renderFlashcards();
}
function tickParity() {
  if (!$('#countdown-display')) return;
  renderCountdown();
  const timer = countdownView();
  const focus = typeof focusView === 'function' ? focusView(data.focus) : null;
  // Prefer actionable ongoing activity in the compact notch.
  const label = $('#closed-clock');
  if (timer.running) {
    const minutes = Math.floor(timer.remaining / 60);
    label.textContent = minutes + ':' + String(timer.remaining % 60).padStart(2,'0');
    label.title = 'Countdown active';
  } else if (focus?.running) {
    label.textContent = Math.floor(focus.remaining / 60) + ':' + String(focus.remaining % 60).padStart(2,'0');
    label.title = 'Focus session active';
  } else {
    label.title = 'Current time';
  }
  if (Date.now() - lastDayRefresh > 30000) {
    lastDayRefresh = Date.now();
    renderDay();
    if (data.health?.day !== localDay()) renderHealth();
  }
}
async function fetchWeather(city) {
  if (weatherInFlight) return;
  const field = $('#weather-current');
  const days = $('#weather-days');
  weatherInFlight = true;
  $('#weather-search').disabled = true;
  field.textContent = 'Looking up ' + city + '…';
  days.replaceChildren();
  try {
    const geoURL = 'https://geocoding-api.open-meteo.com/v1/search?name=' +
      encodeURIComponent(city) + '&count=1&language=en&format=json';
    const geoResponse = await fetch(geoURL);
    if (!geoResponse.ok) throw new Error('Location request failed');
    const geo = await geoResponse.json();
    const place = geo.results?.[0];
    if (!place || !Number.isFinite(place.latitude) || !Number.isFinite(place.longitude)) {
      throw new Error('City not found — try a more specific name');
    }
    const weatherURL = 'https://api.open-meteo.com/v1/forecast?latitude=' + place.latitude +
      '&longitude=' + place.longitude +
      '&current=temperature_2m,relative_humidity_2m,weather_code,wind_speed_10m' +
      '&daily=temperature_2m_max,temperature_2m_min,weather_code&forecast_days=3&timezone=auto';
    const weatherResponse = await fetch(weatherURL);
    if (!weatherResponse.ok) throw new Error('Forecast request failed');
    const forecast = await weatherResponse.json();
    const current = forecast.current;
    if (!current || !Number.isFinite(current.temperature_2m)) throw new Error('Weather not available');
    field.textContent = [place.name,place.admin1,place.country].filter(Boolean).join(', ') +
      ' · ' + current.temperature_2m.toFixed(1) + '°C' +
      ' · Humidity ' + (Number.isFinite(current.relative_humidity_2m) ? current.relative_humidity_2m + '%' : '—') +
      ' · Wind ' + (Number.isFinite(current.wind_speed_10m) ? current.wind_speed_10m + ' km/h' : '—');
    const daily = forecast.daily;
    for (let i=0; daily?.time && i<Math.min(3,daily.time.length); i++) {
      const item = node('div','weather-day');
      item.append(node('b','',daily.time[i]));
      const max = daily.temperature_2m_max?.[i], min = daily.temperature_2m_min?.[i];
      item.append(node('small','',Number.isFinite(min) && Number.isFinite(max) ? min+'–'+max+' °C' : 'Forecast unavailable'));
      days.append(item);
    }
    try { localStorage.setItem('wfs-notch-weather-city',city); } catch {}
  } catch (error) {
    field.textContent = 'Weather unavailable: ' + (error?.message || String(error));
  } finally {
    $('#weather-search').disabled = false;
    weatherInFlight = false;
  }
}
function initParity() {
  if (wellnessInitialized) return;
  wellnessInitialized = true;
  $$('[data-health]').forEach(button => button.addEventListener('click', () => {
    void mutation('health_action', {day: localDay(), action: button.dataset.health});
  }));
  $('#countdown-form').addEventListener('submit', event => {
    event.preventDefault();
    const minutes = Number($('#countdown-minutes').value);
    if (!Number.isInteger(minutes) || minutes < 1 || minutes > 1440) {
      showError('Choose 1–1440 minutes'); return;
    }
    void mutation('countdown_set', {seconds: minutes * 60});
  });
  $('#countdown-toggle').addEventListener('click', () => void mutation('countdown_toggle'));
  $('#countdown-reset').addEventListener('click', () => void mutation('countdown_reset'));
  $$('[data-add-minutes]').forEach(button => button.addEventListener('click', () => {
    void mutation('countdown_adjust', {minutes: Number(button.dataset.addMinutes)});
  }));
  $('#flashcards-note').addEventListener('change', event => {
    flashcardNoteId = event.target.value; flashcardIndex = 0; flashcardShowingAnswer = false; renderFlashcards();
  });
  $('#flashcard-flip').addEventListener('click', () => {
    flashcardShowingAnswer = !flashcardShowingAnswer; renderFlashcards();
  });
  $('#flashcard-next').addEventListener('click', () => {
    const length = cardsFromNote(flashcardNoteId).length;
    if (length) flashcardIndex = (flashcardIndex+1)%length;
    flashcardShowingAnswer = false; renderFlashcards();
  });
  try { $('#weather-city').value = localStorage.getItem('wfs-notch-weather-city') || ''; } catch {}
  $('#weather-form').addEventListener('submit', event => {
    event.preventDefault();
    const city = $('#weather-city').value.trim();
    if (city.length >= 2 && city.length <= 100) void fetchWeather(city);
    else showError('Enter a city name');
  });
  renderParity();
}
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded',initParity,{once:true});
else initParity();
