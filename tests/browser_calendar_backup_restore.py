"""Regression for the preview adapter and backup restore UI contract."""
from pathlib import Path
from shutil import which

from playwright.sync_api import expect, sync_playwright

ROOT = Path(__file__).resolve().parents[1]

with sync_playwright() as pw:
    exe = which('chromium') or which('chromium-browser') or which('google-chrome')
    browser = pw.chromium.launch(**({'executable_path': exe} if exe else {}), headless=True)
    page = browser.new_page(viewport={'width': 760, 'height': 470}, reduced_motion='reduce')
    page.goto((ROOT / 'src/index.html').as_uri())
    page.locator('#expanded').wait_for()
    result = page.evaluate("""async () => {
      const id = 'ics:keep@example.org';
      const original = {...defaults(), events: [{id, title:'Canceled occurrence'}]};
      localStorage.setItem(previewKey, JSON.stringify(original));
      const pre = await previewInvoke('create_backup');
      await previewInvoke('remove_event', {id});
      const post = await previewInvoke('create_backup');
      const protectedState = await previewInvoke('restore_backup', {id:pre.id});
      await previewInvoke('clear_calendar_cancellations');
      const clearedState = await previewInvoke('restore_backup', {id:post.id});
      const newState = await previewInvoke('restore_backup', {id:pre.id});
      return [protectedState.events.length, protectedState.calendar_tombstones.includes(id),
        clearedState.calendar_tombstones.length, newState.events[0]?.id];
    }""")
    assert result == [0, True, 0, 'ics:keep@example.org'], result
    page.get_by_role('button', name='Settings', exact=True).click()
    expect(page.get_by_role('button', name='Restore backup', exact=False).first).to_be_visible()
    browser.close()
print('PASS: backup restore keeps active cancellations and does not undo explicit history reset')
