"""Deterministic Pomodoro browser preview. This is not native resume testing."""
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
    page = browser.new_page(viewport={"width": 760, "height": 470}, reduced_motion="reduce")
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.clock.install()
    page.goto((ROOT / "src" / "index.html").as_uri())
    page.get_by_role("button", name="Focus timer", exact=True).click()
    cycle = page.get_by_role("checkbox", name="Pomodoro cycle")
    action = page.locator("#timer-start")
    status = page.locator("#focus-cycle-status")
    expect(cycle).not_to_be_checked()
    cycle.check()
    expect(status).to_have_text("Focus · 0 / 4 focus sessions this cycle")
    page.get_by_role("button", name="15m", exact=True).click()
    expect(page.locator("#timer-display")).to_have_text("15:00")

    for completed in range(1, 5):
        action.click()
        expect(action).to_have_text("Pause focus")
        expect(cycle).to_be_disabled()
        page.clock.fast_forward(900_250)
        expect(action).to_have_text("Pause break")
        phase = "Long break" if completed == 4 else "Short break"
        expect(status).to_have_text(f"{phase} · {completed} / 4 focus sessions this cycle")
        expect(page.get_by_role("button", name="15m", exact=True)).to_be_disabled()
        if completed == 1:
            action.click()
            expect(action).to_have_text("Resume break")
            page.reload()
            page.get_by_role("button", name="Focus timer", exact=True).click()
            expect(cycle).to_be_checked()
            expect(action).to_have_text("Resume break")
            action.click()
            expect(action).to_have_text("Pause break")
        if completed == 4:
            evidence = ROOT / "test-results"
            evidence.mkdir(exist_ok=True)
            page.screenshot(path=str(evidence / "browser-pomodoro-long-break.png"))
        page.clock.fast_forward(900_250 if completed == 4 else 300_250)
        expect(action).to_have_text("Start focus")
        expect(status).to_have_text(f"Focus · {completed % 4} / 4 focus sessions this cycle")

    # A day of sleep advances only the actual focus and its first break.
    action.click()
    expect(action).to_have_text("Pause focus")
    page.clock.fast_forward(86_400_000)
    expect(action).to_have_text("Start focus")
    expect(status).to_have_text("Focus · 1 / 4 focus sessions this cycle")
    cycle.uncheck()
    expect(status).to_have_text("Single focus session")
    assert not errors, errors
    browser.close()
print("PASS: optional persisted Pomodoro cycles, pause/reload, long break, bounded sleep catch-up")
