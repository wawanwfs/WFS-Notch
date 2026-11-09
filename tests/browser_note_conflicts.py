"""Phase 28A: Notes optimistic revision, explicit conflict recovery and restart-safe drafts.

Chromium validates real browser UI and deterministic mock storage. Native atomicity and
legacy note JSON migration are tested by Rust on Windows CI, not claimed here.
"""
import base64
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


def open_preview(browser, seed=None):
    page = browser.new_page(viewport={"width": 1320, "height": 860}, bypass_csp=True)
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.set_content(html)
    page.evaluate("""(seed) => {
      const storage = { ...seed };
      Object.defineProperty(window, 'localStorage', {configurable:true, value:{
        getItem: key => Object.prototype.hasOwnProperty.call(storage,key) ? storage[key] : null,
        setItem: (key, value) => { storage[key] = String(value); },
        removeItem: key => { delete storage[key]; },
        clear: () => { for (const key of Object.keys(storage)) delete storage[key]; }
      }});
    }""", seed or {})
    for path in ["styles/base.css", "styles/calendar.css", "styles/system.css",
                 "styles/media.css", "styles/refinements.css"]:
        page.add_style_tag(content=(UI / path).read_text(encoding="utf-8"))
    for path in ["app.js", "features/modules.js", "features/wellness.js", "features/calendar.js",
                 "features/system.js", "features/media.js", "features/accessibility.js",
                 "features/displays.js"]:
        page.add_script_tag(content=(UI / path).read_text(encoding="utf-8"))
    page.locator("#expanded").wait_for(state="visible")
    return page, errors


with sync_playwright() as playwright:
    executable = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = playwright.chromium.launch(
        **({"executable_path": executable} if executable else {}),
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"]
    )
    page, errors = open_preview(browser)
    page.locator("#nav-notes").click()
    page.locator("#note-new").click()
    page.locator("#note-title").fill("Original note")
    page.locator("#note-body").fill("First saved draft")
    page.locator("#note-save").click()
    page.wait_for_function("""() => data.notes.length === 1 &&
      data.notes[0].revision >= 1 &&
      data.notes[0].body === 'First saved draft' && pendingDraft === null""")
    note_id = page.evaluate("data.notes[0].id")
    assert page.evaluate("data.notes[0].body") == "First saved draft"

    # Simulate another application window writing to private storage while editor
    # still holds a stale revision. Data on this page remains intentionally stale.
    page.evaluate("""async () => {
      const note = data.notes[0];
      await previewInvoke('update_note', {id:note.id, title:'Remote saved version',
        body:'Remote content must survive', tags:['remote'], expectedRevision:note.revision});
    }""")
    page.locator("#note-body").fill("Unsaved local conflict content")
    page.locator("#note-save").click()
    page.locator("#note-conflict").wait_for(state="visible")
    assert page.locator("#note-body").input_value() == "Unsaved local conflict content"
    assert page.evaluate("() => Boolean(localStorage.getItem(noteDraftKey))")
    assert page.evaluate("""() => JSON.parse(localStorage.getItem(previewKey))
      .notes[0].body""") == "Remote content must survive"
    assert page.evaluate("""async () => {
      try { await previewInvoke('remove_note', {id:data.notes[0].id, expectedRevision:0}); return false; }
      catch { return JSON.parse(localStorage.getItem(previewKey)).notes.length === 1; }
    }""")
    page.locator("#note-conflict-copy").click()
    page.wait_for_function("data.notes.length === 2 && pendingDraft === null")
    assert page.evaluate("""() => data.notes.some(note => note.id !== selectedNoteId &&
      note.body === 'Remote content must survive')""")
    assert page.evaluate("""() => data.notes.some(note => note.id === selectedNoteId &&
      note.body === 'Unsaved local conflict content')""")
    assert page.locator("#note-conflict").is_hidden()
    assert page.evaluate("localStorage.getItem(noteDraftKey)") is None

    # Crash simulation: copy the persisted browser state and unsaved local draft
    # into a fresh JS execution environment without allowing debounce to run.
    page.locator("#note-title").fill("Uncommitted title on restart")
    saved = page.evaluate("""() => ({
      'wfs-notch-browser-preview-v2':localStorage.getItem(previewKey),
      'wfs-notch-local-note-draft-v1':localStorage.getItem(noteDraftKey)
    })""")
    assert saved["wfs-notch-local-note-draft-v1"]
    page2, errors2 = open_preview(browser, saved)
    assert page2.locator("#note-title").input_value() == "Uncommitted title on restart"
    assert page2.evaluate("pendingDraft !== null")
    assert page2.evaluate("selectedNoteId === pendingDraft.id")
    page2.locator("#nav-notes").click()
    page2.locator("#note-save").click()
    page2.wait_for_function("""() => data.notes.some(note =>
      note.title === 'Uncommitted title on restart') && pendingDraft === null""")

    # A restarted draft with a mismatched revision must not be auto-applied.
    conflicting_seed = {
        "wfs-notch-browser-preview-v2": page.evaluate("localStorage.getItem(previewKey)"),
        "wfs-notch-local-note-draft-v1": saved["wfs-notch-local-note-draft-v1"],
    }
    page3, errors3 = open_preview(browser, conflicting_seed)
    # The seed's saved state revision equals the draft revision here; first mutate
    # the persisted note to represent a real concurrent update before a restart.
    changed = page3.evaluate("""async () => {
      const draft = readPendingNote();
      const stored = JSON.parse(localStorage.getItem(previewKey));
      const note = stored.notes.find(item => item.id === draft.id);
      note.revision = (note.revision || 0) + 1;
      note.title = 'Newer stored content';
      localStorage.setItem(previewKey, JSON.stringify(stored));
      return {'wfs-notch-browser-preview-v2':localStorage.getItem(previewKey),
        'wfs-notch-local-note-draft-v1':localStorage.getItem(noteDraftKey)};
    }""")
    page4, errors4 = open_preview(browser, changed)
    page4.locator("#nav-notes").click()
    page4.locator("#note-conflict").wait_for(state="visible")
    assert page4.locator("#note-title").input_value() == "Uncommitted title on restart"
    page4.on("dialog", lambda dialog: dialog.accept())
    page4.locator("#note-conflict-reload").click()
    page4.wait_for_function("""() => pendingDraft === null && noteConflictDraft === null""")
    assert page4.locator("#note-title").input_value() == "Newer stored content"
    assert page4.evaluate("localStorage.getItem(noteDraftKey)") is None

    assert not (errors + errors2 + errors3 + errors4), "JS exceptions: " + str(
        errors + errors2 + errors3 + errors4)
    browser.close()
print("PASS: optimistic revisions, stale delete reject, explicit conflict copy, restart draft, explicit reload")
