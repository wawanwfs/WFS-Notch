"""UI-only browser smoke test. Does NOT verify Windows/Tauri or Rust compilation.

Runs Chromium against in-memory HTML/CSS/JS because the sandbox may block file/http navigation.
The preview-mode localStorage is supplied by an isolated, deterministic test adapter.
"""
from pathlib import Path
import base64
from shutil import which
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / 'src'
EVIDENCE = ROOT / 'test-results'
EVIDENCE.mkdir(parents=True, exist_ok=True)

with sync_playwright() as playwright:
    browser_path = which('chromium') or which('chromium-browser') or which('google-chrome')
    browser = playwright.chromium.launch(
        **({'executable_path': browser_path} if browser_path else {}),
        headless=True,
        args=['--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu'],
    )
    context = browser.new_context(viewport={'width': 1400, 'height': 850}, bypass_csp=True)
    page = context.new_page()
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    html = (UI / 'index.html').read_text().replace('<script src="app.js" defer></script>', '').replace('<script src="features/modules.js" defer></script>', '')
    html = html.replace('<link rel="stylesheet" href="styles/base.css">', '')
    encoded_logo = base64.b64encode((UI / 'assets' / 'wfs-notch-mark.svg').read_bytes()).decode('ascii')
    html = html.replace('src="assets/wfs-notch-mark.svg"', f'src="data:image/svg+xml;base64,{encoded_logo}"')
    page.set_content(html)
    page.evaluate("""() => {
      const store = {};
      Object.defineProperty(window,'localStorage',{ configurable:true, value: {
        getItem: (k) => Object.prototype.hasOwnProperty.call(store,k) ? store[k] : null,
        setItem: (k,v) => { store[k] = String(v); },
        removeItem: (k) => { delete store[k]; },
        clear: () => { for (const k of Object.keys(store)) delete store[k]; }
      }});
    }""")
    page.add_style_tag(content=(UI / 'styles/base.css').read_text())
    page.add_script_tag(content=(UI / 'app.js').read_text())
    page.locator('#expanded').wait_for(state='visible', timeout=5000)
    assert page.locator('#task-count').inner_text() == '0'
    page.screenshot(path=str(EVIDENCE / 'browser-home.png'))

    page.get_by_role('button', name='Tasks', exact=True).click()
    page.locator('#todo-input').fill('Review release checklist')
    page.get_by_role('button', name='Add task').click()
    page.get_by_text('Review release checklist').wait_for()
    assert page.locator('#task-count').inner_text() == '1'
    page.get_by_role('button', name='Complete: Review release checklist').click()
    assert page.locator('#task-count').inner_text() == '0'
    page.get_by_role('button', name='Delete task: Review release checklist').click()
    assert page.locator('#task-count').inner_text() == '0'

    page.get_by_role('button', name='Notes', exact=True).click()
    page.get_by_role('button', name='+ New note').click()
    page.locator('#note-title').fill('Daily log')
    page.locator('#note-body').fill('Persist this note through the local adapter.')
    page.get_by_role('button', name='Save now').click()
    page.get_by_text('Saved locally', exact=True).wait_for()
    page.get_by_role('button', name='+ New note').click()
    page.locator('#note-title').fill('Second note')
    page.locator('#note-body').fill('Another note')
    page.get_by_role('button', name='Save now').click()
    page.get_by_text('Saved locally', exact=True).wait_for()
    page.locator('.note-card').filter(has_text='Daily log').click()
    assert page.locator('#note-title').input_value() == 'Daily log'
    assert page.locator('#note-body').input_value() == 'Persist this note through the local adapter.'
    page.screenshot(path=str(EVIDENCE / 'browser-notes.png'))

    page.get_by_role('button', name='Focus timer').click()
    page.get_by_role('button', name='15m').click()
    assert page.locator('#timer-display').inner_text() == '15:00'
    page.get_by_role('button', name='Start focus').click()
    assert page.get_by_role('button', name='Pause focus').is_visible()
    page.get_by_role('button', name='Pause focus').click()
    assert page.get_by_role('button', name='Resume focus').is_visible()
    page.get_by_role('button', name='Reset').click()
    assert page.locator('#timer-display').inner_text() == '15:00'

    page.get_by_role('button', name='Settings').click()
    page.locator('#hover-toggle').uncheck()
    assert page.locator('#hover-toggle').is_checked() is False
    page.locator('#pin-toggle').check()
    assert page.locator('#pin-toggle').is_checked() is True
    assert page.locator('#pin-button').get_attribute('aria-pressed') == 'true'
    page.get_by_role('button', name='Exit app').click()
    page.get_by_text('Exit is available in the installed desktop app').wait_for()

    assert not errors, f'Browser exceptions: {errors}'
    print('PASS: browser UI smoke (todos, note autosave, note selection, focus timer, settings, preview exit)')
    print('Evidence:', EVIDENCE / 'browser-home.png', EVIDENCE / 'browser-notes.png')
    browser.close()
