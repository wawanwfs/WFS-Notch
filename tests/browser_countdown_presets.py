"""User-editable countdown presets remain local across reload, reset and backup."""
from pathlib import Path
from shutil import which

from playwright.sync_api import expect, sync_playwright

ROOT = Path(__file__).resolve().parents[1]

with sync_playwright() as pw:
    executable = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(
        **({"executable_path": executable} if executable else {}),
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"],
    )
    page = browser.new_page(viewport={"width": 560, "height": 420}, reduced_motion="reduce")
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.on("dialog", lambda dialog: dialog.dismiss())
    page.goto((ROOT / "src" / "index.html").as_uri())
    page.get_by_role("button", name="Countdown", exact=True).click()
    group = page.get_by_role("group", name="Countdown presets")
    expect(group.get_by_role("button")).to_have_text(["5 min", "15 min", "30 min"])
    page.get_by_text("Edit countdown presets", exact=True).click()
    slots = page.locator("[data-edit-countdown-slot]")
    for i, minutes in enumerate(["3", "7", "19"]):
        slots.nth(i).fill(minutes)
    page.get_by_role("button", name="Save timer presets").click()
    expect(page.locator("#countdown-preset-status")).to_have_text("Timer presets saved locally.")
    expect(group.get_by_role("button")).to_have_text(["3 min", "7 min", "19 min"])
    expect(page.locator("#countdown-display")).to_have_text("05:00")
    group.get_by_role("button", name="7 min").click()
    expect(page.locator("#countdown-display")).to_have_text("07:00")
    page.locator("#countdown-toggle").click()
    expect(page.locator("#countdown-toggle")).to_have_text("Pause")
    group.get_by_role("button", name="19 min").click()
    expect(page.locator("#countdown-toggle")).to_have_text("Pause")
    page.locator("#countdown-reset").click()
    expect(page.locator("#countdown-display")).to_have_text("05:00")
    page.reload()
    page.get_by_role("button", name="Countdown", exact=True).click()
    expect(group.get_by_role("button")).to_have_text(["3 min", "7 min", "19 min"])
    assert page.evaluate("""async () => {
      await previewInvoke('create_backup');
      await previewInvoke('set_countdown_presets', {presets:[1, 2, 3]});
      const [backup] = await previewInvoke('list_backups');
      const restored = await previewInvoke('restore_backup', {id:backup.id});
      return restored.countdown.preset_minutes.join(',') === '3,7,19';
    }""")
    assert not errors, errors
    browser.close()
print("PASS: editable countdown presets, running replacement consent, reset/reload and backup")
