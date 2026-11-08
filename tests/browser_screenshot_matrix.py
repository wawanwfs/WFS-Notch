"""Phase 12 screenshot evidence for every module at desktop DPI and narrow viewports.

Headless Chromium validates browser layout, not actual Windows/Tauri behavior.
Snapshots are uploaded as CI artifacts. They are not pixel-golden parity claims.
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
html = html.replace('src="assets/wfs-notch-mark.svg"',
                    'src="data:image/svg+xml;base64,' + logo + '"')
css_files = [
    "styles/base.css", "styles/calendar.css", "styles/system.css",
    "styles/media.css", "styles/refinements.css", "styles/displays.css"
]
js_files = [
    "app.js", "features/modules.js", "features/wellness.js",
    "features/calendar.js", "features/system.js", "features/media.js",
    "features/accessibility.js", "features/displays.js"
]

with sync_playwright() as pw:
    executable = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(
        **({"executable_path": executable} if executable else {}),
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"]
    )
    # Actual Windows DPI/monitor tests are tracked separately: these are CSS/WebView proxies.
    for width, height, dpi in [(760, 470, 1), (760, 470, 1.25),
                               (760, 470, 1.5), (560, 420, 1)]:
        context = browser.new_context(
            viewport={"width": width, "height": height},
            device_scale_factor=dpi, bypass_csp=True,
            reduced_motion="reduce",
        )
        page = context.new_page()
        errors = []
        page.on("pageerror", lambda err: errors.append(str(err)))
        page.set_content(html)
        page.evaluate("""() => {
          const cache = {};
          Object.defineProperty(window, 'localStorage', { configurable:true, value:{
            getItem:k => cache[k] ?? null,
            setItem:(k,v) => {cache[k] = String(v);},
            removeItem:k => {delete cache[k];}
          }});
        }""")
        for name in css_files:
            page.add_style_tag(content=(UI / name).read_text(encoding="utf-8"))
        for name in js_files:
            page.add_script_tag(content=(UI / name).read_text(encoding="utf-8"))
        page.locator("#expanded").wait_for(state="visible")
        tabs = page.locator(".tabs .tab")
        assert tabs.count() == 16, "The original 16 UI modules must remain available"
        for index in range(tabs.count()):
            tab = tabs.nth(index)
            module = tab.get_attribute("data-tab")
            tab.click()
            page.locator("#panel-" + module + ".active").wait_for(state="visible")
            assert page.evaluate(
                "document.documentElement.scrollWidth <= innerWidth + 1"
            ), "Unexpected horizontal viewport overflow: " + module
            assert page.locator("#panel-" + module).evaluate(
                "(node) => node.scrollWidth <= node.clientWidth + 2"
            ), "Clipped module content: " + module
            name = f"browser-phase12-{module}-{width}x{height}-{round(dpi*100)}dpi.png"
            page.screenshot(path=str(EVIDENCE / name))
        assert not errors, "JavaScript errors: " + "; ".join(errors)
        context.close()
    browser.close()
print("PASS: 16 module screenshots at 100%, 125%, 150% and narrow viewport (64 total)")
