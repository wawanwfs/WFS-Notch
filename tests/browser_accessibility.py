"""WFS Notch desktop UI smoke regression for keyboard navigation and reduced motion.
This browser preview does not simulate Windows media or hardware state.
"""
from pathlib import Path
from shutil import which
import base64
import re
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "src"
EVIDENCE = ROOT / "test-results"
EVIDENCE.mkdir(exist_ok=True)

html = (UI / "index.html").read_text(encoding="utf-8")
html = re.sub(r'<script src="[^"]+" defer></script>', "", html)
html = re.sub(r'<link rel="stylesheet" href="[^"]+">', "", html)
logo = base64.b64encode((UI / "assets" / "wfs-notch-mark.svg").read_bytes()).decode()
html = html.replace(
    'src="assets/wfs-notch-mark.svg"',
    'src="data:image/svg+xml;base64,' + logo + '"',
)

with sync_playwright() as pw:
    exe = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(
        **({"executable_path": exe} if exe else {}),
        headless=True,
        args=["--no-sandbox", "--disable-dev-shm-usage"],
    )
    page = browser.new_page(viewport={"width": 760, "height": 470}, bypass_csp=True)
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.set_content(html)
    page.evaluate("""() => {
      const cache = {};
      Object.defineProperty(window, 'localStorage', { configurable: true, value: {
        getItem: key => cache[key] ?? null,
        setItem: (key,value) => { cache[key] = String(value); },
        removeItem: key => { delete cache[key]; }
      }});
    }""")
    for filename in (
        "styles/base.css", "styles/calendar.css", "styles/system.css",
        "styles/media.css", "styles/refinements.css", "styles/displays.css"
    ):
        page.add_style_tag(content=(UI / filename).read_text(encoding="utf-8"))
    for filename in (
        "app.js", "features/modules.js", "features/wellness.js", "features/calendar.js",
        "features/system.js", "features/media.js", "features/accessibility.js", "features/displays.js"
    ):
        page.add_script_tag(content=(UI / filename).read_text(encoding="utf-8"))

    page.locator("#expanded").wait_for(state="visible")
    assert page.locator("#display-monitor").is_disabled()
    assert page.locator("#display-apply").is_disabled()
    assert "cannot detect real monitors" in page.locator("#display-status").inner_text()
    assert page.locator(".tabs .tab").count() >= 14
    assert page.locator("#notification-test").is_disabled(), "Browser must not simulate a Windows toast"
    assert page.locator("#notification-scheduler-refresh").is_disabled(), "Browser must not invent native scheduler status"
    assert "installed Windows desktop app" in page.locator("#notification-scheduler-status").inner_text()
    assert not page.locator("#notifications-toggle").is_checked(), "Sensitive toast capability must be opt-in"
    assert page.locator("#notification-details-toggle").is_disabled()
    assert page.locator("#notifications-pause-toggle").is_disabled()
    page.locator("#nav-settings").click()
    page.locator("#notifications-toggle").check()
    page.wait_for_function("document.querySelector('#notification-details-toggle').disabled === false")
    assert not page.locator("#notification-details-toggle").is_checked()
    page.locator("#notifications-pause-toggle").check()
    page.wait_for_function("document.querySelector('#notification-test').disabled === true")
    page.locator("#notifications-pause-toggle").uncheck()
    page.locator("#notifications-toggle").uncheck()
    page.wait_for_function("document.querySelector('#notification-details-toggle').disabled === true")
    assert page.locator("#notification-test").is_disabled()
    assert page.locator("#notifications-pause-toggle").is_disabled()
    page.locator("#on-top-toggle").uncheck()
    page.wait_for_function("document.querySelector('#on-top-toggle').checked === false")
    page.locator("#on-top-toggle").check()
    page.locator("#nav-home").click()
    assert page.locator("#nav-home").get_attribute("aria-current") == "page"
    assert page.locator("#nav-todos").get_attribute("aria-controls") == "panel-todos"

    page.locator("#nav-home").focus()
    page.keyboard.press("ArrowRight")
    page.locator("#panel-todos.active").wait_for()
    assert page.locator("#nav-todos").get_attribute("aria-pressed") == "true"
    assert page.locator("#nav-todos").evaluate("(node) => document.activeElement === node")
    page.keyboard.press("Home")
    page.locator("#panel-home.active").wait_for()
    page.keyboard.press("End")
    page.locator("#panel-settings.active").wait_for()
    page.keyboard.press("Alt+6")
    page.locator("#panel-media.active").wait_for()
    assert page.locator("#media-track-title").inner_text() == "No active media session"
    assert "Browser preview" in page.locator("#media-track-source").inner_text()
    assert page.locator("#media-player-picker").is_disabled(), "Browser must not fake Windows media sessions"
    assert page.locator("#media-artwork").is_hidden(), "No artwork without a real Windows media session"
    page.evaluate("renderMediaDetails({title:'Fixture', artwork_data_url:'data:image/svg+xml;base64,PHN2Zz4=', position_ms:0, duration_ms:1000, playback_status:'Paused'})")
    assert page.locator("#media-artwork").is_hidden(), "Reject SVG and unknown image types"
    page.evaluate("renderMediaDetails(null)")

    page.locator("#nav-notes").click()
    page.locator("#panel-notes.active").wait_for()
    page.locator("#note-search").focus()
    page.keyboard.press("Alt+1")
    assert page.locator("#panel-notes.active").count() == 1, "Shortcut must not steal focus from text input"

    assert page.evaluate("document.documentElement.scrollWidth <= window.innerWidth")
    assert page.evaluate("getComputedStyle(document.querySelector('.notch')).borderTopWidth === '1px'")
    page.emulate_media(reduced_motion="reduce")
    assert page.evaluate(
        "parseFloat(getComputedStyle(document.querySelector('.signal span')).animationDuration) < .01"
    ), "Reduced-motion preference must stop pulsing animation"
    page.locator("#nav-home").click()
    page.locator("#panel-home.active").wait_for()
    page.screenshot(path=str(EVIDENCE / "browser-ux-v080.png"))
    assert not errors, "Unhandled JavaScript errors: " + "; ".join(errors)
    browser.close()
print("PASS: keyboard navigation, text-input shortcut protection, media preview honesty, reduced motion, no page overflow")
