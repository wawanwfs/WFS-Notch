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
const weatherGroups = new Map([
  [0, 'Clear'], [1, 'Mainly clear'], [2, 'Partly cloudy'], [3, 'Overcast'],
  [45, 'Fog'], [48, 'Fog'], [51, 'Drizzle'], [53, 'Drizzle'], [55, 'Drizzle'],
  [56, 'Freezing drizzle'], [57, 'Freezing drizzle'], [61, 'Rain'], [63, 'Rain'], [65, 'Heavy rain'],
  [66, 'Freezing rain'], [67, 'Freezing rain'], [71, 'Snow'], [73, 'Snow'], [75, 'Heavy snow'],
  [77, 'Snow grains'], [80, 'Rain showers'], [81, 'Rain showers'], [82, 'Heavy showers'],
  [85, 'Snow showers'], [86, 'Snow showers'], [95, 'Thunderstorm'], [96, 'Thunderstorm with hail'],
  [99, 'Thunderstorm with hail'],
]);

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
const weatherNumber = (value, unit, digits = 0) =>
  Number.isFinite(value) ? `${value.toFixed(digits)}${unit}` : '—';
const weatherTime = value => typeof value === 'string' && /^\d{4}-\d{2}-\d{2}T(?:[01]\d|2[0-3]):[0-5]\d$/.test(value);
const weatherDate = value => typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value);

function renderWeatherForecast(place, forecast) {
  const {current, hourly, daily} = forecast;
  const details = $('#weather-details');
  const hours = $('#weather-hours');
  const days = $('#weather-days');
  const condition = weatherGroups.get(current.weather_code);
  $('#weather-current').textContent = [place.name, place.admin1, place.country].filter(Boolean).join(', ') +
    ' · ' + weatherNumber(current.temperature_2m, '°C', 1) + (condition ? ` · ${condition}` : '');
  const sunTime = values => weatherTime(values?.[0]) ? values[0].slice(11) : '—';
  const detailItems = [
    ['Feels like', weatherNumber(current.apparent_temperature, '°C', 1)],
    ['Humidity', weatherNumber(current.relative_humidity_2m, '%')],
    ['Wind', weatherNumber(current.wind_speed_10m, ' km/h', 1)],
    ['Pressure', weatherNumber(current.surface_pressure, ' hPa')],
    ['Sunrise', sunTime(daily?.sunrise)], ['Sunset', sunTime(daily?.sunset)],
  ];
  details.replaceChildren(...detailItems.map(([name, value]) => {
    const group = node('div');
    group.append(node('dt', '', name), node('dd', '', value));
    return group;
  }));

  const hourlyTimes = Array.isArray(hourly?.time) ? hourly.time.slice(0, 168) : [];
  const now = weatherTime(current.time) ? current.time : null;
  if (now) {
    const next = hourlyTimes.findIndex(time => weatherTime(time) && time >= now);
    for (let i = next; i >= 0 && i < hourlyTimes.length && i < next + 12; i++) {
      if (!weatherTime(hourlyTimes[i])) continue;
      const item = node('div', 'weather-day');
      const rain = hourly.precipitation_probability?.[i];
      const rainText = Number.isFinite(rain) && rain >= 0 && rain <= 100 ? ` · Rain ${rain}%` : '';
      const stamp = hourlyTimes[i];
      item.append(node('b', '', stamp.slice(0, 10) === now.slice(0, 10) ? stamp.slice(11) : stamp.slice(5).replace('T', ' ')),
        node('small', '', weatherNumber(hourly.temperature_2m?.[i], '°C', 1) + rainText));
      hours.append(item);
    }
  }
  if (!hours.childElementCount) hours.append(node('p', 'event-hint', 'Hourly forecast unavailable.'));

  const dailyTimes = Array.isArray(daily?.time) ? daily.time.slice(0, 7) : [];
  for (const [i, date] of dailyTimes.entries()) {
    if (!weatherDate(date)) continue;
    const item = node('div', 'weather-day');
    item.append(node('b', '', date === now?.slice(0, 10) ? `Today · ${date}` : date),
      node('small', '', `Low ${weatherNumber(daily.temperature_2m_min?.[i], '°C')} · High ${weatherNumber(daily.temperature_2m_max?.[i], '°C')}`));
    const description = weatherGroups.get(daily.weather_code?.[i]);
    if (description) item.append(node('small', '', description));
    days.append(item);
  }
  if (!days.childElementCount) days.append(node('p', 'event-hint', 'Daily forecast unavailable.'));
  $('#weather-timezone').textContent = typeof forecast.timezone === 'string' && /^[\w/+\-]{1,64}$/.test(forecast.timezone)
    ? `Times shown in ${forecast.timezone.replaceAll('_', ' ')}.` : 'Times are local to the selected city.';
  $('#weather-forecast').hidden = false;
}

const validWeatherPlace = place => place && typeof place.name === 'string' && place.name.trim() &&
  Number.isFinite(place.latitude) && Number.isFinite(place.longitude) &&
  Math.abs(place.latitude) <= 90 && Math.abs(place.longitude) <= 180;

async function fetchWeather(city, selectedPlace = null) {
  if (weatherInFlight) return;
  const field = $('#weather-current');
  if (selectedPlace) field.focus({preventScroll: true});
  weatherInFlight = true;
  $('#weather-search').disabled = true;
  field.textContent = 'Looking up ' + (selectedPlace?.name || city) + '…';
  $('#weather-forecast').hidden = true;
  $('#weather-hours').replaceChildren();
  $('#weather-days').replaceChildren();
  const locations = $('#weather-locations');
  locations.hidden = true;
  locations.replaceChildren();
  try {
    const signal = AbortSignal.timeout(12000);
    let place = selectedPlace;
    if (!place) {
      const geoURL = 'https://geocoding-api.open-meteo.com/v1/search?name=' +
        encodeURIComponent(city) + '&count=5&language=en&format=json';
      const geoResponse = await fetch(geoURL, {signal});
      if (!geoResponse.ok) throw new Error('Location request failed');
      const geo = await geoResponse.json();
      const matches = Array.isArray(geo.results) ? geo.results.slice(0, 5).filter(validWeatherPlace) : [];
      if (!matches.length) throw new Error('City not found — try a more specific name');
      if (matches.length > 1) {
        field.textContent = 'Choose the city you want to check.';
        for (const match of matches) {
          const label = [match.name, match.admin2, match.admin1, match.country].filter(Boolean).join(', ');
          const button = node('button', 'secondary weather-location', label);
          button.type = 'button';
          button.addEventListener('click', () => void fetchWeather(city, match));
          locations.append(button);
        }
        locations.hidden = false;
        return;
      }
      [place] = matches;
    }
    const weatherURL = 'https://api.open-meteo.com/v1/forecast?latitude=' + place.latitude +
      '&longitude=' + place.longitude +
      '&current=temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,wind_speed_10m,surface_pressure' +
      '&hourly=temperature_2m,precipitation_probability' +
      '&daily=temperature_2m_max,temperature_2m_min,weather_code,sunrise,sunset&forecast_days=7&timezone=auto';
    const weatherResponse = await fetch(weatherURL, {signal});
    if (!weatherResponse.ok) throw new Error('Forecast request failed');
    const forecast = await weatherResponse.json();
    const current = forecast.current;
    if (!current || !Number.isFinite(current.temperature_2m)) throw new Error('Weather not available');
    renderWeatherForecast(place, forecast);
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
