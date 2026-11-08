"""Calendar and system Chromium preview tests. This does NOT validate Windows/Rust."""
from pathlib import Path
import base64
from shutil import which
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / 'src'
EVIDENCE = ROOT / 'test-results'
with sync_playwright() as pw:
    browser_path = which('chromium') or which('chromium-browser') or which('google-chrome')
    browser = pw.chromium.launch(**({'executable_path': browser_path} if browser_path else {}), headless=True,
        args=['--no-sandbox', '--disable-dev-shm-usage'])
    context = browser.new_context(viewport={'width': 1440, 'height': 900}, bypass_csp=True)
    page = context.new_page()
    exceptions = []
    page.on('pageerror', lambda error: exceptions.append(str(error)))
    page.on('dialog', lambda dialog: dialog.accept())
    html = (UI / 'index.html').read_text().replace('<link rel="stylesheet" href="styles/base.css">', '').replace('<script src="app.js" defer></script>', '').replace('<script src="features/modules.js" defer></script>', '')
    logo = base64.b64encode((UI / 'assets' / 'wfs-notch-mark.svg').read_bytes()).decode()
    html = html.replace('src="assets/wfs-notch-mark.svg"', f'src="data:image/svg+xml;base64,{logo}"')
    page.set_content(html)
    page.evaluate('''() => { const data = {}; Object.defineProperty(window, 'localStorage', { configurable:true, value:{
      getItem:k => data[k] ?? null, setItem:(k,v) => {data[k] = String(v)}, removeItem:k => {delete data[k]}
    }}); }''')
    page.add_style_tag(content=(UI / 'styles/base.css').read_text())
    page.add_script_tag(content=(UI / 'app.js').read_text())
    page.locator('#expanded').wait_for(state='visible')

    page.get_by_role('button', name='Calendar', exact=True).click()
    assert page.locator('#panel-calendar').is_visible()
    page.locator('#event-title').fill('Client review')
    page.locator('#event-time').fill('2027-06-14T11:30')
    page.locator('#event-remind').select_option('15')
    page.get_by_role('button', name='Add event').click()
    page.get_by_text('Client review', exact=True).wait_for()
    assert '1 EVENT' in page.locator('#calendar-count').inner_text()
    assert 'Client review' in page.locator('#next-event').inner_text()
    page.get_by_role('button', name='Edit event: Client review').click()
    assert page.locator('#event-title').input_value() == 'Client review'
    page.locator('#event-title').fill('Updated client review')
    page.get_by_role('button', name='Save changes').click()
    page.get_by_text('Updated client review', exact=True).wait_for()
    assert not page.get_by_text('Client review', exact=True).count()
    page.wait_for_timeout(400)
    page.screenshot(path=str(EVIDENCE / 'browser-calendar-phase3.png'))

    page.get_by_role('button', name='Tasks', exact=True).click()
    page.locator('#todo-input').fill('Create before backup')
    page.get_by_role('button', name='Add task').click()
    page.get_by_text('Create before backup').wait_for()
    page.get_by_role('button', name='Settings').click()
    page.get_by_role('button', name='Create backup').click()
    page.get_by_text('1 tasks · 0 notes · 1 events').wait_for()

    page.get_by_role('button', name='Tasks', exact=True).click()
    page.get_by_role('button', name='Delete task: Create before backup').click()
    page.locator('#todo-input').fill('Delete on restore')
    page.get_by_role('button', name='Add task').click()
    page.get_by_text('Delete on restore').wait_for()
    page.get_by_role('button', name='Calendar', exact=True).click()
    page.get_by_role('button', name='Delete event: Updated client review').click()
    page.locator('#calendar-count').get_by_text('0 EVENTS', exact=True).wait_for()
    page.get_by_role('button', name='Settings').click()
    page.locator('.backup-entry button').click()
    page.get_by_text('Backup restored successfully.').wait_for()
    page.get_by_role('button', name='Tasks', exact=True).click()
    page.get_by_text('Create before backup').wait_for()
    assert not page.get_by_text('Delete on restore').count()
    page.get_by_role('button', name='Calendar', exact=True).click()
    page.get_by_text('Updated client review', exact=True).wait_for()

    page.get_by_role('button', name='System', exact=True).click()
    page.get_by_text('System data is available in the native Rust/Windows app only.').wait_for()
    assert page.locator('#system-cpu').inner_text() == '—'
    assert page.locator('#system-memory').inner_text() == '—'
    page.wait_for_timeout(400)
    page.screenshot(path=str(EVIDENCE / 'browser-system-phase3.png'))

    # Reminder set to 5 min before an event in 2 min must display while app is open.
    page.get_by_role('button', name='Calendar', exact=True).click()
    page.locator('#event-title').fill('In-app reminder check')
    page.evaluate("""() => {
      const d = new Date(Date.now() + 120000);
      const pad = n => String(n).padStart(2, '0');
      document.querySelector('#event-time').value = `${d.getFullYear()}-${pad(d.getMonth()+1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
    }""")
    page.locator('#event-remind').select_option('5')
    page.get_by_role('button', name='Add event').click()
    page.wait_for_function("document.querySelector('#toast').textContent.includes('Calendar reminder: In-app reminder check')", timeout=5000)
    page.get_by_role('button', name='Delete event: In-app reminder check').click()
    page.get_by_text('1 EVENT', exact=True).wait_for()

    assert not exceptions, f'Browser errors: {exceptions}'
    print('PASS: Phase 3 calendar CRUD, in-app reminders, restore, preview telemetry honesty, UI errors: 0')
    browser.close()
