"""Daily/weekly task interaction with deterministic time and local persistence."""
from datetime import datetime, timezone
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
    page = browser.new_page(viewport={"width": 760, "height": 470}, timezone_id="UTC")
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.clock.set_fixed_time(datetime(2027, 6, 19, 9, 0, tzinfo=timezone.utc))
    page.goto((ROOT / "src" / "index.html").as_uri())
    page.get_by_role("button", name="Tasks", exact=True).click()
    page.get_by_role("textbox", name="New task", exact=True).fill("Check backups")
    page.locator("#todo-recurrence").select_option("daily")
    page.get_by_role("button", name="Add task").click()
    expect(page.locator("#toast")).to_contain_text("need a due date")
    page.locator("#todo-due").fill("2027-06-16T09:00")
    page.locator("#todo-priority").select_option("high")
    page.get_by_role("button", name="Add task").click()
    complete = page.get_by_role("button", name="Complete and reschedule: Check backups")
    expect(complete).to_be_visible()
    original = page.evaluate("data.todos[0].id")
    complete.click()
    expect(page.locator("#toast")).to_contain_text("next deadline is scheduled")
    assert page.evaluate("data.todos[0].due_at_ms") == 1813482000000  # 2027-06-20 09:00 UTC
    assert page.evaluate("data.todos[0].id") == original
    assert page.evaluate("data.todos[0].priority") == "high"
    assert not page.evaluate("data.todos[0].done")
    page.reload()
    page.get_by_role("button", name="Tasks", exact=True).click()
    expect(complete).to_be_visible()
    page.get_by_role("button", name="Edit task: Check backups").click()
    expect(page.locator("#todo-edit-recurrence")).to_have_value("daily")
    expect(page.locator("#todo-edit-due")).to_have_value("2027-06-20T09:00")
    page.locator("#todo-edit-recurrence").select_option("weekly")
    page.get_by_role("button", name="Save task").click()
    expect(page.locator(".todo-metadata")).to_contain_text("Every 7 days")
    complete.click()
    page.wait_for_function("new Date(data.todos[0].due_at_ms).toISOString() === '2027-06-27T09:00:00.000Z'")
    assert page.evaluate("new Date(data.todos[0].due_at_ms).toISOString()") == "2027-06-27T09:00:00.000Z"
    assert not errors, errors
    browser.close()
print("PASS: recurring task validation, deadline catch-up, weekly edit and reload persistence")
