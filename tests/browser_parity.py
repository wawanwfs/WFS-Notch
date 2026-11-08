"""Phase 5 browser-only tests: functional UI, no fake claims about native Windows."""
from pathlib import Path
import base64
from shutil import which
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "src"
EVIDENCE = ROOT / "test-results"

with sync_playwright() as pw:
    path = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(**({"executable_path": path} if path else {}),
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"])
    page = browser.new_page(viewport={"width": 1440, "height": 960}, bypass_csp=True)
    problems = []
    page.on("pageerror", lambda error: problems.append(str(error)))
    page.on("dialog", lambda dialog: dialog.accept())
    html = (UI / "index.html").read_text()
    html = html.replace('<link rel="stylesheet" href="styles.css">', "")
    html = html.replace('<script src="app.js" defer></script>', "")
    html = html.replace('<script src="modules.js" defer></script>', "")
    html = html.replace('<script src="parity.js" defer></script>', "")
    mark = base64.b64encode((UI / "assets/wfs-notch-mark.svg").read_bytes()).decode()
    html = html.replace('src="assets/wfs-notch-mark.svg"', f'src="data:image/svg+xml;base64,{mark}"')
    page.set_content(html)
    page.evaluate("""() => {
      const state = {};
      Object.defineProperty(window,'localStorage',{configurable:true,value:{
        getItem:key=>state[key] ?? null,
        setItem:(key,value)=>{state[key]=String(value)},
        removeItem:key=>{delete state[key]}
      }});
    }""")
    page.add_style_tag(content=(UI / "styles.css").read_text())
    for name in ("app.js", "modules.js", "parity.js"):
        page.add_script_tag(content=(UI / name).read_text())
    page.locator("#expanded").wait_for(state="visible")

    page.get_by_role("button", name="Health", exact=True).click()
    page.get_by_role("button", name="+ Glass").click()
    page.wait_for_function("document.querySelector('#health-water').textContent === '1 / 8'")
    page.get_by_role("button", name="Log short break").click()
    page.wait_for_function("document.querySelector('#health-breaks').textContent === '1'")
    page.get_by_role("button", name="+ 1 mindful minute").click()
    page.wait_for_function("document.querySelector('#health-calm').textContent === '1'")

    page.get_by_role("button", name="Countdown", exact=True).click()
    page.locator("#countdown-minutes").fill("2")
    page.get_by_role("button", name="Set duration").click()
    page.wait_for_function("document.querySelector('#countdown-display').textContent === '02:00'")
    page.locator("#countdown-toggle").click()
    page.wait_for_function("document.querySelector('#countdown-toggle').textContent === 'Pause'")
    page.get_by_role("button", name="+5 minutes").click()
    page.wait_for_function("document.querySelector('#countdown-display').textContent.startsWith('06:')")
    page.locator("#countdown-toggle").click()
    page.wait_for_function("document.querySelector('#countdown-toggle').textContent !== 'Pause'")

    page.get_by_role("button", name="Day Progress", exact=True).click()
    assert page.locator("#day-percent").inner_text().endswith("%")
    assert "tasks completed" in page.locator("#day-task-progress").inner_text()

    page.get_by_role("button", name="Notes", exact=True).click()
    page.locator("#note-new").click()
    page.locator("#note-body").fill("First question :: First answer\nSecond question :: Second answer")
    page.locator("#note-save").click()
    page.wait_for_function("document.querySelector('#save-status').textContent.includes('Saved')")
    page.get_by_role("button", name="Flashcards", exact=True).click()
    page.wait_for_function("document.querySelector('#flashcards-count').textContent === '2 CARDS'")
    assert page.locator("#flashcard-face").inner_text() == "First question"
    page.get_by_role("button", name="Show answer").click()
    assert page.locator("#flashcard-face").inner_text() == "First answer"
    page.get_by_role("button", name="Next card").click()
    assert page.locator("#flashcard-face").inner_text() == "Second question"

    # Deterministic mocked API responses, not claims about live weather availability.
    page.evaluate("""() => {
        window.fetch = async url => ({
            ok: true,
            json: async () => url.includes('geocoding-api')
              ? {results:[{name:'Testville',country:'Testland',latitude:1,longitude:2}]}
              : {current:{temperature_2m:27.5,relative_humidity_2m:60,wind_speed_10m:3},
                 daily:{time:['2026-11-07'],temperature_2m_min:[24],temperature_2m_max:[30]}}
        });
    }""")
    page.get_by_role("button", name="Weather", exact=True).click()
    page.locator("#weather-city").fill("Testville")
    page.get_by_role("button", name="Get weather").click()
    page.wait_for_function("document.querySelector('#weather-current').textContent.includes('Testville')")
    assert "27.5°C" in page.locator("#weather-current").inner_text()

    page.get_by_role("button", name="Health", exact=True).click()
    assert page.locator("#health-water").inner_text() == "1 / 8"
    page.screenshot(path=str(EVIDENCE / "browser-parity-phase5.png"))
    assert not problems, f"JavaScript errors: {problems}"
    print("PASS Phase 5: persisted health, countdown, day progress, flashcards, weather UI mock")
    browser.close()
