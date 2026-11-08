"""Browser regression test for the monthly calendar and Notes search.
Native import/export is tested in Rust unit tests, not faked in browser preview.
"""
import base64
from pathlib import Path
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
html = html.replace('src="assets/wfs-notch-mark.svg"', 'src="data:image/svg+xml;base64,' + logo + '"')

with sync_playwright() as pw:
    exe = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(
        **({"executable_path": exe} if exe else {}),
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"],
    )
    page = browser.new_page(viewport={"width": 1360, "height": 900}, bypass_csp=True)
    issues = []
    page.on("pageerror", lambda error: issues.append(str(error)))
    page.set_content(html)
    page.evaluate("""() => {
      const data = {};
      Object.defineProperty(window, 'localStorage', { configurable:true, value: {
        getItem: k => data[k] ?? null,
        setItem: (k,v) => { data[k] = String(v); },
        removeItem: k => { delete data[k]; }
      }});
    }""")
    for name in ["styles/base.css", "styles/calendar.css", "styles/system.css", "styles/media.css"]:
        page.add_style_tag(content=(UI / name).read_text(encoding="utf-8"))
    for name in ["app.js", "features/modules.js", "features/wellness.js", "features/calendar.js", "features/system.js", "features/media.js"]:
        page.add_script_tag(content=(UI / name).read_text(encoding="utf-8"))
    page.locator("#expanded").wait_for(state="visible")

    page.get_by_role("button", name="Notes", exact=True).click()
    page.get_by_role("button", name="+ New note").click()
    page.locator("#note-title").fill("Architecture Roadmap")
    page.locator("#note-body").fill("Rust / Tauri calendar interoperability design.")
    page.get_by_role("button", name="Save now").click()
    page.get_by_text("Saved locally", exact=True).wait_for()
    page.locator("#note-tags").fill("Release, ARCH, release")
    page.get_by_role("button", name="Save now").click()
    page.wait_for_function("""() => data.notes.some(note =>
      note.title === 'Architecture Roadmap' &&
      JSON.stringify(note.tags) === JSON.stringify(['release', 'arch']))""")
    page.evaluate("""async () => { data = await previewInvoke('load_data'); render(); }""")
    assert page.evaluate("() => JSON.stringify(data.notes[0].tags)") == '["release","arch"]'
    page.locator("#note-search").fill("RELEASE")
    page.get_by_role("button", name="Architecture Roadmap").wait_for(state="visible")
    page.locator("#note-search").clear()
    page.locator("#note-tag-filter").select_option("arch")
    page.get_by_role("button", name="Architecture Roadmap").wait_for(state="visible")
    page.locator("#note-tag-filter").select_option("")
    assert page.evaluate("""async () => {
      const note = data.notes[0];
      try { await previewInvoke('update_note', {id:note.id,title:note.title,
        body:note.body,tags:['<script>']}); return false; }
      catch { return data.notes[0].tags.includes('release'); }
    }""")
    page.locator("#note-search").fill("INTEROPERABILITY")
    page.get_by_role("button", name="Architecture Roadmap").wait_for(state="visible")
    page.locator("#note-search").fill("no matching phrase here")
    page.get_by_text("No matching notes.").wait_for()
    page.locator("#note-search").clear()
    page.get_by_role("button", name="Architecture Roadmap").wait_for(state="visible")

    page.get_by_role("button", name="Calendar", exact=True).click()
    assert page.locator(".calendar-day").count() >= 28
    current = page.locator("#calendar-month-title").inner_text()
    page.locator("#calendar-next").click()
    assert page.locator("#calendar-month-title").inner_text() != current
    page.locator("#calendar-prev").click()
    assert page.locator("#calendar-month-title").inner_text() == current
    page.locator("#calendar-today").click()
    day = page.locator(".calendar-day.today").first
    assert day.count() == 1
    day.click()
    assert page.locator("#event-time").input_value()
    assert page.locator(".calendar-day.selected").count() == 1
    page.locator("#event-title").fill("Monthly Badge Test")
    page.locator("#event-remind").select_option("off")
    page.get_by_role("button", name="Add event").click()
    page.get_by_text("Monthly Badge Test", exact=True).wait_for()
    assert page.locator(".calendar-day.has-event").count() >= 1
    assert page.locator(".calendar-day.has-event small").first.inner_text() == "1"
    page.locator(".ics-box summary").click()
    page.locator("#ics-import-button").click()
    page.get_by_text("ICS import requires the installed WFS Notch desktop app.").wait_for()
    assert "requires the native Windows" in page.locator("#ics-status").inner_text()
    page.screenshot(path=str(EVIDENCE / "browser-calendar-plus.png"))
    page.get_by_role("button", name="System", exact=True).click()
    assert "unavailable in browser preview" in page.locator("#system-disks").inner_text()
    assert "unavailable in browser preview" in page.locator("#system-networks").inner_text()
    page.get_by_role("button", name="Media", exact=True).click()
    assert "No active media session" in page.locator("#media-track-title").inner_text()
    assert "Browser preview" in page.locator("#media-track-source").inner_text()
    assert not issues, f"JavaScript errors: {issues}"
    browser.close()
print("PASS: calendar month navigation/event badges, Notes full-text search, honest browser ICS preview")
