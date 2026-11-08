"""Phase 15 Chromium-only all-day and recurrence-series UI regression.

Native Windows WebView2, toast visibility, monitor hotplug and GSMTC are
NOT tested by this browser preview test.
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
      Object.defineProperty(window, 'localStorage', { configurable:true, value: {
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
    page.locator("#event-all-day").check()
    page.locator("#event-title").fill("Company retreat")
    page.locator("#event-start-date").fill("2027-06-14")
    page.locator("#event-end-date").fill("2027-06-17")
    assert page.locator("#event-time").is_hidden()
    assert page.locator("#event-remind").is_disabled()
    page.get_by_role("button", name="Add event").click()
    page.get_by_text("Company retreat", exact=True).wait_for()
    assert page.evaluate("""() => {
      const e = data.events.find(e => e.title === 'Company retreat');
      return e.all_day.start_date === '2027-06-14' &&
        e.all_day.end_date_exclusive === '2027-06-17' &&
        e.remind_minutes === null;
    }""")
    assert page.locator(".calendar-day.has-event").count() == 3
    assert "2027-06-14 – 2027-06-16 · All day" in page.locator("#event-list").inner_text()
    page.get_by_role("button", name="Edit event: Company retreat").click()
    assert page.locator("#event-all-day").is_checked()
    assert page.locator("#event-start-date").input_value() == "2027-06-14"
    assert page.locator("#event-end-date").input_value() == "2027-06-17"
    page.locator("#event-end-date").fill("2027-06-18")
    page.get_by_role("button", name="Save changes").click()
    assert page.locator(".calendar-day.has-event").count() == 4
    page.screenshot(path=str(EVIDENCE / "browser-calendar-phase15-all-day.png"))

    # Imported series instances are represented in the native model; browser
    # fixture exercises the actual UI and mock local-only mutation callbacks.
    page.evaluate("""() => {
      for (const day of [20, 21]) data.events.push({
        id: 'ics:series:' + day, series_uid: 'ics:series',
        title: 'Recurring alert', starts_at_ms: Date.UTC(2027, 5, day, 12),
        remind_minutes: null, description: '', created_ms: Date.now()
      });
      // The browser preview persists through localStorage, not the live JS
      // data variable. Hydrate both stores before exercising IPC-style writes.
      localStorage.setItem(previewKey, JSON.stringify(data));
      render();
    }""")
    assert page.get_by_role("button", name="Rename entire series: Recurring alert").count() == 2
    page.get_by_role("button", name="Rename entire series: Recurring alert").first.click()
    assert page.locator("#event-time").is_disabled()
    page.locator("#event-title").fill("Updated series")
    page.get_by_role("button", name="Rename entire series", exact=True).click()
    updated = page.get_by_role("button", name="Rename entire series: Updated series")
    updated.first.wait_for(state="visible")
    assert updated.count() == 2
    page.get_by_role("button", name="Delete entire series: Updated series").first.click()
    updated.first.wait_for(state="detached")
    assert updated.count() == 0
    assert page.get_by_text("Company retreat", exact=True).count() == 1
    assert not issues, f"Calendar all-day/series browser errors: {issues}"
    browser.close()

print("PASS: Chromium Phase15 all-day date-only create/edit, 4-day month badges, series title/cancel UI; native Windows NOT TESTED")
