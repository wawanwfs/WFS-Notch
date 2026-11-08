"""Phase 22A series-shift UI smoke, no fake native recurrence execution."""
from pathlib import Path
import re, base64
from shutil import which
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "src"
html = (UI / "index.html").read_text(encoding="utf-8")
html = re.sub(r'<script src="[^"]+" defer></script>', "", html)
html = re.sub(r'<link rel="stylesheet" href="[^"]+">', "", html)
logo = base64.b64encode((UI / "assets/wfs-notch-mark.svg").read_bytes()).decode()
html = html.replace('src="assets/wfs-notch-mark.svg"', 'src="data:image/svg+xml;base64,' + logo + '"')
with sync_playwright() as pw:
    exe = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(**({"executable_path": exe} if exe else {}),
                                 headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"])
    page = browser.new_page(viewport={"width": 1100, "height": 900}, bypass_csp=True)
    errors = []
    page.on("pageerror", lambda e: errors.append(str(e)))
    page.set_content(html)
    page.evaluate("""() => {
      const store = {};
      Object.defineProperty(window, 'localStorage', {configurable:true,value:{
        getItem: key => store[key] ?? null,
        setItem: (key,value) => { store[key] = String(value); },
        removeItem: key => { delete store[key]; }
      }});
    }""")
    for css in ["styles/base.css", "styles/calendar.css", "styles/refinements.css"]:
        page.add_style_tag(content=(UI / css).read_text(encoding="utf-8"))
    for path in ["app.js", "features/modules.js", "features/wellness.js", "features/calendar.js"]:
        page.add_script_tag(content=(UI / path).read_text(encoding="utf-8"))
    page.locator("#expanded").wait_for(state="visible")
    page.get_by_role("button", name="Calendar", exact=True).click()
    page.evaluate("""() => {
      const uid = 'ics:safe-shift@example.org';
      const stamp = Date.UTC(2027, 5, 14, 9);
      data.events = [{id:uid+':'+stamp, title:'Safe UTC series', starts_at_ms:stamp,
        ends_at_ms:stamp+3600000, remind_minutes:15, description:'Kept private',
        created_ms:123, all_day:null, series_uid:uid}];
      data.calendar_series = {[uid]: {schema_version:1,uid,dtstart:['DTSTART','20270614T090000Z'],
        dtend:['DTEND','20270614T100000Z'],rrule:'FREQ=DAILY;COUNT=1',
        exdates:[],rdates:[],modified_occurrence_ids:[],title:'Safe UTC series',description:'Kept private'}};
      render();
    }""")
    page.get_by_role("button", name="Preview whole-series day shift: Safe UTC series").click()
    assert page.locator("#calendar-series-shift").is_visible()
    assert page.locator("#calendar-shift-apply").is_disabled()
    page.locator("#calendar-shift-days").fill("3")
    page.get_by_role("button", name="Preview changes").click()
    assert "requires native windows" in page.locator("#calendar-shift-status").inner_text().lower()
    assert page.locator("#calendar-shift-apply").is_disabled()
    assert page.evaluate("() => data.events[0].starts_at_ms === Date.UTC(2027,5,14,9)")
    page.get_by_role("button", name="Cancel", exact=True).last.click()
    assert page.locator("#calendar-series-shift").is_hidden()
    assert page.evaluate("() => data.events[0].description === 'Kept private'")
    assert not errors, f"browser JS errors: {errors}"
    browser.close()
print("PASS: Phase22A series shift preview controls, honest native-only scope, no browser fake mutation; real native QA NOT TESTED")
