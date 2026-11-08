"""Desktop module preview regressions; hardware/media/tray require installed Windows app."""
from pathlib import Path
import base64
from shutil import which
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "src"
EVIDENCE = ROOT / "test-results"
EVIDENCE.mkdir(exist_ok=True)

with sync_playwright() as pw:
    executable = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(**({"executable_path": executable} if executable else {}),
                                  headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"])
    context = browser.new_context(viewport={"width": 1440, "height": 950}, bypass_csp=True)
    page = context.new_page()
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.on("dialog", lambda dialog: dialog.accept())
    html = (UI / "index.html").read_text()
    html = html.replace('<link rel="stylesheet" href="styles/base.css">', "")
    html = html.replace('<script src="app.js" defer></script>', "")
    html = html.replace('<script src="features/modules.js" defer></script>', "")
    logo = base64.b64encode((UI / "assets/wfs-notch-mark.svg").read_bytes()).decode()
    html = html.replace('src="assets/wfs-notch-mark.svg"', 'src="data:image/svg+xml;base64,' + logo + '"')
    page.set_content(html)
    page.evaluate("""() => {
      const storage = {};
      Object.defineProperty(window, 'localStorage', { configurable: true, value: {
        getItem: key => storage[key] ?? null,
        setItem: (key, value) => { storage[key] = String(value); },
        removeItem: key => { delete storage[key]; },
      }});
    }""")
    page.add_style_tag(content=(UI / "styles/base.css").read_text())
    page.add_script_tag(content=(UI / "app.js").read_text())
    page.add_script_tag(content=(UI / "features/modules.js").read_text())
    page.locator("#expanded").wait_for(state="visible")
    page.get_by_role("button", name="Habits", exact=True).click()
    page.locator("#habit-title").fill("Read 20 minutes")
    page.get_by_role("button", name="Add habit").click()
    page.get_by_text("Read 20 minutes", exact=True).wait_for()
    page.get_by_role("button", name="Complete today: Read 20 minutes").click()
    page.get_by_text("1 OF 1 TODAY").wait_for()
    page.get_by_role("button", name="Dock", exact=True).click()
    page.locator("#quicklink-label").fill("WFS docs")
    page.locator("#quicklink-url").fill("https://example.com/help")
    page.get_by_role("button", name="Pin link").click()
    page.get_by_role("button", name="Open WFS docs").wait_for()
    # Unsafe schemes cannot be added through the preview backend.
    bad = page.evaluate("""async () => {
      try { await previewInvoke('add_quick_link', {label:'bad', url:'javascript:alert(1)'}); return 'accepted'; }
      catch { return 'rejected'; }
    }""")
    assert bad == "rejected"
    page.get_by_role("button", name="Media", exact=True).click()
    page.get_by_role("button", name="Play or pause").click()
    page.get_by_text("This Windows integration requires the installed desktop application").wait_for()
    page.get_by_role("button", name="Tools", exact=True).click()
    page.locator("#math-input").fill("120 * 1.15")
    page.locator("#math-form button").click()
    assert page.locator("#math-output").inner_text() == "138"
    page.locator("#stopwatch-toggle").click()
    page.wait_for_timeout(350)
    page.locator("#stopwatch-lap").click()
    page.locator("#stopwatch-toggle").click()
    assert "Lap 1" in page.locator("#stopwatch-laps").inner_text()
    page.locator("#password-generate").click()
    assert len(page.locator("#password-output").inner_text()) == 20
    page.keyboard.press("Control+k")
    page.locator("#palette-search").fill("Notes")
    page.get_by_role("button", name="Notes", exact=True).last.click()
    assert page.locator("#panel-notes").is_visible()
    page.get_by_role("button", name="Settings", exact=True).click()
    assert page.locator("#autostart-toggle").is_disabled()
    page.get_by_role("button", name="Create backup").click()
    page.get_by_role("button", name="Habits", exact=True).click()
    page.get_by_role("button", name="Delete habit: Read 20 minutes").click()
    page.get_by_role("button", name="Dock", exact=True).click()
    page.get_by_role("button", name="Unpin WFS docs").click()
    page.get_by_role("button", name="Settings", exact=True).click()
    page.locator(".backup-entry button").click()
    page.get_by_text("Backup restored successfully.").wait_for()
    page.get_by_role("button", name="Habits", exact=True).click()
    page.get_by_text("Read 20 minutes", exact=True).wait_for()
    page.get_by_role("button", name="Dock", exact=True).click()
    page.get_by_role("button", name="Open WFS docs").wait_for()
    page.screenshot(path=str(EVIDENCE / "browser-phase4.png"))
    assert not errors, errors
    print("PASS: Phase 4 habit checkins, quick links, URL rejection, media preview honesty, tools, command palette, backup roundtrip")
    browser.close()
