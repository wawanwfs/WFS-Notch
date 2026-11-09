"""Phase28A2 browser test: user-driven Notes JSON transfer and no-clobber import."""
import base64
import json
from pathlib import Path
import re
from shutil import which
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "src"
html = (UI / "index.html").read_text(encoding="utf-8")
html = re.sub(r'<script src="[^"]+" defer></script>', "", html)
html = re.sub(r'<link rel="stylesheet" href="[^"]+">', "", html)
logo = base64.b64encode((UI / "assets" / "wfs-notch-mark.svg").read_bytes()).decode()
html = html.replace('src="assets/wfs-notch-mark.svg"', 'src="data:image/svg+xml;base64,' + logo + '"')

with sync_playwright() as playwright:
    exe = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = playwright.chromium.launch(
        **({"executable_path": exe} if exe else {}),
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"]
    )
    page = browser.new_page(viewport={"width": 1280, "height": 860}, bypass_csp=True)
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.set_content(html)
    page.evaluate("""() => {
      const storage = {};
      Object.defineProperty(window, 'localStorage', {configurable:true,value:{
        getItem: key => Object.prototype.hasOwnProperty.call(storage,key) ? storage[key] : null,
        setItem: (key,value) => {storage[key] = String(value);},
        removeItem: key => {delete storage[key];}
      }});
    }""")
    for path in ["styles/base.css", "styles/calendar.css", "styles/system.css",
                 "styles/media.css", "styles/refinements.css"]:
        page.add_style_tag(content=(UI / path).read_text(encoding="utf-8"))
    for path in ["app.js", "features/modules.js", "features/wellness.js", "features/calendar.js",
                 "features/system.js", "features/media.js", "features/accessibility.js",
                 "features/displays.js"]:
        page.add_script_tag(content=(UI / path).read_text(encoding="utf-8"))
    page.locator("#expanded").wait_for(state="visible")
    page.locator("#nav-notes").click()
    page.locator("#note-new").click()
    page.locator("#note-title").fill("Existing note")
    page.locator("#note-body").fill("Important original text")
    page.locator("#note-tags").fill("work")
    page.locator("#note-save").click()
    page.wait_for_function("""() => data.notes.length === 1 &&
      data.notes[0].body === 'Important original text' && pendingDraft === null""")

    page.locator("#notes-transfer summary").click()
    page.locator("#notes-export-button").click()
    page.wait_for_function("document.querySelector('#notes-export-json').value.includes('wfs-notch-notes')")
    exported = json.loads(page.locator("#notes-export-json").input_value())
    assert exported["version"] == 1
    assert exported["format"] == "wfs-notch-notes"
    assert len(exported["notes"]) == 1
    original_id = exported["notes"][0]["id"]
    original_revision = exported["notes"][0]["revision"]

    # Bad JSON and unknown versions must never enable the import action.
    page.locator("#notes-import-json").fill("{broken")
    page.locator("#notes-import-preview").click()
    assert page.locator("#notes-import-apply").is_disabled()
    assert "rejected" in page.locator("#notes-transfer-status").inner_text()

    injected = {
        "id": "import-note-001", "title": "New imported note",
        "body": "<script>window.NOT_ALLOWED=true</script> literal text",
        "updated_ms": 1800000000000, "tags": ["imported"], "revision": 0
    }
    payload = {
        "format": "wfs-notch-notes", "version": 1,
        "notes": [
            {**exported["notes"][0], "title": "Malicious collision overwrite"},
            injected
        ]
    }
    page.locator("#notes-import-json").fill(json.dumps(payload))
    page.locator("#notes-import-preview").click()
    page.wait_for_function("notesImportPreview?.new_count === 1")
    assert "1 new note" in page.locator("#notes-transfer-status").inner_text()
    assert "1 existing" in page.locator("#notes-transfer-status").inner_text()
    assert page.locator("#notes-import-apply").is_enabled()
    page.on("dialog", lambda dialog: dialog.accept())
    page.locator("#notes-import-apply").click()
    page.wait_for_function("data.notes.length === 2 && notesImportPreview === null")
    original = page.evaluate("(id) => data.notes.find(item => item.id === id)", original_id)
    assert original["title"] == "Existing note"
    assert original["body"] == "Important original text"
    assert original["revision"] == original_revision
    assert page.evaluate("""() => data.notes.find(note =>
      note.id === 'import-note-001').body.includes('<script>')""")
    assert not page.evaluate("window.NOT_ALLOWED === true")
    assert page.locator("#notes-import-apply").is_disabled()

    # Reimport must skip all IDs and never change the existing note or revision.
    page.locator("#notes-import-preview").click()
    page.wait_for_function("notesImportPreview?.new_count === 0")
    assert page.locator("#notes-import-apply").is_disabled()
    assert "2 existing" in page.locator("#notes-transfer-status").inner_text()
    page.evaluate("""async () => {
      data = await previewInvoke('import_notes_json', {
        payload:document.querySelector('#notes-import-json').value
      });
    }""")
    assert page.evaluate("data.notes.length") == 2
    assert page.evaluate("(id) => data.notes.find(note => note.id === id).revision", original_id) == original_revision

    # Duplicate IDs in one incoming bundle are rejected atomically.
    duplicate = {"format": "wfs-notch-notes", "version": 1,
                 "notes": [injected, injected]}
    page.locator("#notes-import-json").fill(json.dumps(duplicate))
    page.locator("#notes-import-preview").click()
    assert page.locator("#notes-import-apply").is_disabled()
    assert "rejected" in page.locator("#notes-transfer-status").inner_text()
    assert page.evaluate("data.notes.length") == 2
    assert not errors, "Unhandled JavaScript errors: " + str(errors)
    browser.close()
print("PASS: local Notes JSON export, preview validation, append-only import, idempotence and safe literal text")
