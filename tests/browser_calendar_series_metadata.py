"""Phase 17 Chromium-only source-series metadata UI / scoped edit regression.

Simulated state is for frontend interaction only. Native RFC5545 import and
series persistence are proven separately in Rust tests. Windows WebView2
interactive, GSMTC and OS toast/monitor QA remain NOT TESTED.
"""
from pathlib import Path
import re
import base64
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
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"]
    )
    page = browser.new_page(viewport={"width": 1360, "height": 900}, bypass_csp=True)
    issues = []
    page.on("pageerror", lambda err: issues.append(str(err)))
    page.on("dialog", lambda dlg: dlg.accept())
    page.set_content(html)
    page.evaluate("""() => {
      const saved = {};
      Object.defineProperty(window, 'localStorage', {configurable: true, value: {
        getItem: k => saved[k] ?? null,
        setItem: (k, v) => { saved[k] = String(v); },
        removeItem: k => { delete saved[k]; }
      }});
    }""")
    for css in ["styles/base.css", "styles/calendar.css", "styles/refinements.css"]:
        page.add_style_tag(content=(UI / css).read_text(encoding="utf-8"))
    for script in ["app.js", "features/modules.js", "features/wellness.js", "features/calendar.js"]:
        page.add_script_tag(content=(UI / script).read_text(encoding="utf-8"))
    page.locator("#expanded").wait_for(state="visible")
    page.get_by_role("button", name="Calendar", exact=True).click()

    page.evaluate("""() => {
      const uid = 'ics:phase17@example.org';
      data.events.push(
        {id: uid + ':1', series_uid: uid, title: 'Weekly review',
         starts_at_ms: Date.UTC(2027, 5, 14, 9), ends_at_ms: Date.UTC(2027, 5, 14, 10),
         remind_minutes: null, description: '', created_ms: Date.now()},
        {id: uid + ':2', series_uid: uid, title: 'Weekly review',
         starts_at_ms: Date.UTC(2027, 5, 21, 9), ends_at_ms: Date.UTC(2027, 5, 21, 10),
         remind_minutes: null, description: '', created_ms: Date.now()}
      );
      data.calendar_series = {[uid]: {
        schema_version: 1, uid, title: 'Weekly review', description: '',
        dtstart: ['DTSTART', '20270614T090000Z'],
        dtend: ['DTEND', '20270614T100000Z'],
        duration: null,
        rrule: 'FREQ=WEEKLY;BYDAY=MO;COUNT=2',
        exdates: [], rdates: [], floating_timezone: null,
        modified_occurrence_ids: []
      }};
      localStorage.setItem(previewKey, JSON.stringify(data));
      render();
    }""")
    assert page.get_by_text("Imported series · saved rule: FREQ=WEEKLY;BYDAY=MO;COUNT=2").count() == 2
    page.get_by_role("button", name="Edit event: Weekly review").first.click()
    page.locator("#event-description").fill("Individually edited")
    page.get_by_role("button", name="Save changes").click()
    page.wait_for_function("""() => {
      const source = data.calendar_series['ics:phase17@example.org'];
      return source.modified_occurrence_ids.length === 1 &&
        data.events.filter(e => e.description === 'Individually edited').length === 1;
    }""")
    page.get_by_role("button", name="Rename entire series: Weekly review").first.click()
    page.locator("#event-title").fill("Renamed whole series")
    page.get_by_role("button", name="Rename entire series", exact=True).click()
    page.get_by_role("button", name="Rename entire series: Renamed whole series").first.wait_for(state="visible")
    assert page.evaluate("""() => {
      const source = data.calendar_series['ics:phase17@example.org'];
      return source.title === 'Renamed whole series' &&
        source.modified_occurrence_ids.length === 1 &&
        data.events.every(e => e.title === 'Renamed whole series');
    }""")
    page.screenshot(path=str(EVIDENCE / "browser-calendar-phase17-series-metadata.png"))
    page.get_by_role("button", name="Delete entire series: Renamed whole series").first.click()
    page.get_by_role("button", name="Delete entire series: Renamed whole series").first.wait_for(state="detached")
    assert page.evaluate("""() => {
      const persisted = JSON.parse(localStorage.getItem(previewKey));
      return data.calendar_tombstones.includes('ics:phase17@example.org') &&
        !data.calendar_series['ics:phase17@example.org'] &&
        !persisted.calendar_series['ics:phase17@example.org'];
    }""")
    assert not issues, f"Browser errors: {issues}"
    browser.close()

print("PASS: Chromium Phase17 source metadata, one-occurrence edit, whole-series rename/cancel and local preview parity; native Windows NOT TESTED")
