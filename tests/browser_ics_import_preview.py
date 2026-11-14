"""Validate the browser/IPC contract. Rust owns ICS validation and storage."""
from pathlib import Path
import re
from shutil import which

from playwright.sync_api import expect, sync_playwright

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "src"

with sync_playwright() as pw:
    executable = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(
        **({"executable_path": executable} if executable else {}),
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"],
    )
    page = browser.new_page(viewport={"width": 1100, "height": 760}, reduced_motion="reduce", bypass_csp=True)
    page.on("console", lambda message: print(message.text) if message.type == "error" else None)
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    html = re.sub(r'<script src="[^"]+" defer></script>', '', (UI / 'index.html').read_text())
    html = re.sub(r'<link rel="stylesheet" href="[^"]+">', '', html)
    page.set_content(html)
    page.evaluate("""() => {
      const storage = new Map();
      Object.defineProperty(window, 'localStorage', {value: {
        getItem: key => storage.get(key) ?? null,
        setItem: (key, value) => storage.set(key, String(value)),
        removeItem: key => storage.delete(key)
      }});
      window.icsCalls = [];
      window.rejectIcsApply = false;
      window.__TAURI__ = {core: {invoke: async (command, payload) => {
        if (command === 'load_data') {
          const state = defaults();
          state.settings.pin_open = true;
          return state;
        }
        if (command === 'list_backups') return [];
        if (command === 'preview_ics_import') {
          window.icsCalls.push([command, payload]);
          return {new_events: 0, removed_events: 1, fingerprint: '9abc123400009abc',
            cancellations: [{title: '<b>Planning</b>', original: '20270615T090000Z',
              original_kind: 'RECURRENCE-ID', removes_local_event: true}]};
        }
        if (command === 'import_ics') {
          window.icsCalls.push([command, payload]);
          if (window.rejectIcsApply) throw new Error('Preview stale');
          const state = defaults();
          state.settings.pin_open = true;
          return state;
        }
        return null;
      }}};
    }""")
    for name in ('base.css', 'calendar.css', 'refinements.css'):
        page.add_style_tag(content=(UI / 'styles' / name).read_text())
    for name in ('app.js', 'features/modules.js', 'features/wellness.js', 'features/calendar.js'):
        page.add_script_tag(content=(UI / name).read_text())
    page.locator("#expanded").wait_for(state="visible")
    assert page.evaluate("Boolean(nativeInvoke)"), "native adapter fixture was not installed"
    page.get_by_role("button", name="Calendar", exact=True).click()
    page.get_by_text("Import / export iCalendar (.ics)", exact=True).click()
    source = page.locator("#ics-import-data")
    preview = page.locator("#ics-import-preview")
    apply = page.locator("#ics-import-button")
    status = page.locator("#ics-status")
    canceled = page.locator("#ics-cancellation-preview")
    expect(apply).to_be_disabled()
    source.fill("BEGIN:VCALENDAR\nEND:VCALENDAR")
    preview.click()
    expect(status).to_contain_text("Preview only", timeout=3000)
    expect(apply).to_be_enabled()
    expect(status).to_contain_text("1 saved occurrence(s) removed")
    expect(canceled).to_contain_text("<b>Planning</b> · 2027-06-15 09:00:00 (UTC)")
    expect(canceled.locator("b")).to_have_count(0)
    assert page.evaluate("icsCalls.map(x => x[0])") == ["preview_ics_import"]
    source.fill("BEGIN:VCALENDAR\nVERSION:2.0\nEND:VCALENDAR")
    expect(apply).to_be_disabled()
    expect(canceled).to_be_hidden()
    preview.click()
    apply.click()
    expect(status).to_contain_text("removed 1 saved occurrence(s)")
    assert page.evaluate("icsCalls.at(-1)[1].expectedFingerprint") == "9abc123400009abc"
    assert "VERSION:2.0" in page.evaluate("icsCalls.at(-1)[1].icsText")
    page.evaluate("window.rejectIcsApply = true")
    preview.click()
    apply.click()
    expect(status).to_contain_text("Import not applied")
    expect(apply).to_be_disabled()
    assert not errors, errors
    browser.close()
print("PASS: ICS preview is read-only, cancellation details are text, edits and stale apply invalidate review")
