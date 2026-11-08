"""Phase 16 Chromium-only calendar time-range and cancellation regression.

This does NOT test native WebView2, Windows notifications, real multi-monitor
DPI, Windows 10/11 interactive hardware or ICS import (implemented in Rust).
"""
from pathlib import Path
import base64
import re
from shutil import which
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "src"
EVIDENCE = ROOT / "test-results"
EVIDENCE.mkdir(exist_ok=True)
html = (UI / "index.html").read_text(encoding="utf-8")
html = re.sub(r'<script src="[^"]+" defer></script>', "", html)
html = re.sub(r'<link rel="stylesheet" href="[^"]+">', "", html)
logo = base64.b64encode((UI / "assets" / "wfs-notch-mark.svg").read_bytes()).decode()
html = html.replace('src="assets/wfs-notch-mark.svg"', "src=\"data:image/svg+xml;base64," + logo + "\"")

with sync_playwright() as pw:
    exe = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(
        **({"executable_path": exe} if exe else {}),
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"],
    )
    page = browser.new_page(viewport={"width": 1360, "height": 900}, bypass_csp=True)
    issues = []
    page.on("pageerror", lambda error: issues.append(str(error)))
    page.on("dialog", lambda dialog: dialog.accept())
    page.set_content(html)
    page.evaluate("""() => {
      const saved = {};
      Object.defineProperty(window, 'localStorage', { configurable: true, value: {
        getItem: k => saved[k] ?? null,
        setItem: (k,v) => { saved[k] = String(v); },
        removeItem: k => { delete saved[k]; }
      }});
    }""")
    for css in ["styles/base.css", "styles/calendar.css", "styles/refinements.css"]:
        page.add_style_tag(content=(UI / css).read_text(encoding="utf-8"))
    for js in ["app.js", "features/modules.js", "features/wellness.js", "features/calendar.js"]:
        page.add_script_tag(content=(UI / js).read_text(encoding="utf-8"))
    page.locator("#expanded").wait_for(state="visible")
    page.get_by_role("button", name="Calendar", exact=True).click()
    page.evaluate("() => { calendarMonth = new Date(2027, 5, 1); renderCalendarPlus(); }")

    page.locator("#event-title").fill("Multi-day conference")
    page.locator("#event-description").fill("Private agenda; no external sync.")
    page.locator("#event-time").fill("2027-06-14T09:00")
    page.locator("#event-end-time").fill("2027-06-16T11:00")
    page.get_by_role("button", name="Add event").click()
    page.get_by_text("Multi-day conference", exact=True).wait_for()
    assert page.evaluate("""() => {
      const event = data.events.find(e => e.title === 'Multi-day conference');
      return event && event.ends_at_ms > event.starts_at_ms &&
        event.description === 'Private agenda; no external sync.' && !event.all_day;
    }""")
    assert page.locator(".calendar-day.has-event").count() == 3

    page.get_by_role("button", name="Edit event: Multi-day conference").click()
    assert page.locator("#event-end-time").input_value() == "2027-06-16T11:00"
    assert page.locator("#event-description").input_value() == "Private agenda; no external sync."
    page.locator("#event-end-time").fill("2027-06-15T10:00")
    page.locator("#event-description").fill("Updated private notes.")
    page.get_by_role("button", name="Save changes").click()
    page.wait_for_function("""() => {
      const event = data.events.find(e => e.title === 'Multi-day conference');
      return !!event && event.description === 'Updated private notes.';
    }""")
    assert page.locator(".calendar-day.has-event").count() == 2
    page.screenshot(path=str(EVIDENCE / "browser-calendar-phase16-timed-range.png"))

    # Recreate a normalized imported recurring series in the local preview store.
    # The real RFC5545 import path is tested only by the Rust unit suite.
    page.evaluate("""() => {
      for (const day of [20, 21]) data.events.push({
        id: 'ics:cancel@example.org:' + day, series_uid: 'ics:cancel@example.org',
        title: 'Cancelable ICS', starts_at_ms: Date.UTC(2027, 5, day, 9),
        ends_at_ms: Date.UTC(2027, 5, day, 10), remind_minutes: null,
        description: '', created_ms: Date.now()
      });
      localStorage.setItem(previewKey, JSON.stringify(data));
      render();
    }""")
    page.get_by_role("button", name="Delete entire series: Cancelable ICS").first.click()
    page.get_by_role("button", name="Delete entire series: Cancelable ICS").first.wait_for(state="detached")
    assert page.evaluate("""() => {
      const persisted = JSON.parse(localStorage.getItem(previewKey));
      return data.calendar_tombstones.includes('ics:cancel@example.org') &&
        persisted.calendar_tombstones.includes('ics:cancel@example.org') &&
        data.events.every(e => e.series_uid !== 'ics:cancel@example.org');
    }""")

    page.locator("details.ics-box summary").click()
    page.get_by_role("button", name="Allow re-import of deleted ICS events").click()
    page.wait_for_function("() => data.calendar_tombstones.length === 0")
    assert page.evaluate("""() => {
      const persisted = JSON.parse(localStorage.getItem(previewKey));
      return persisted.calendar_tombstones.length === 0 &&
        data.events.some(e => e.title === 'Multi-day conference');
    }""")
    assert not issues, f"Phase16 browser errors: {issues}"
    browser.close()

print("PASS: Chromium Phase16 timed DTSTART/DTEND editing, local multi-day grid, private description and explicit ICS cancellation reset; native Windows NOT TESTED")
