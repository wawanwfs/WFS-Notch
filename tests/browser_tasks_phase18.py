"""Phase18 Workstream F — local tasks metadata, editing, persistence, and accessibility.

Headless Chromium preview only; no native WebView2/Windows QA claims.
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
mark = base64.b64encode((UI / "assets" / "wfs-notch-mark.svg").read_bytes()).decode()
html = html.replace('src="assets/wfs-notch-mark.svg"', "src=\"data:image/svg+xml;base64," + mark + "\"")

with sync_playwright() as pw:
    exe = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(
        **({"executable_path": exe} if exe else {}), headless=True,
        args=["--no-sandbox", "--disable-dev-shm-usage"],
    )
    page = browser.new_page(viewport={"width": 1360, "height": 900}, bypass_csp=True)
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.set_content(html)
    page.evaluate("""() => {
      const values = {};
      Object.defineProperty(window, 'localStorage', {configurable: true, value: {
        getItem: key => values[key] ?? null,
        setItem: (key, value) => { values[key] = String(value); },
        removeItem: key => { delete values[key]; }
      }});
    }""")
    for css in ["styles/base.css", "styles/calendar.css", "styles/refinements.css"]:
        page.add_style_tag(content=(UI / css).read_text(encoding="utf-8"))
    for script in ["app.js", "features/modules.js", "features/wellness.js", "features/calendar.js"]:
        page.add_script_tag(content=(UI / script).read_text(encoding="utf-8"))

    page.locator("#expanded").wait_for(state="visible")
    page.get_by_role("button", name="Tasks", exact=True).click()
    # The browser cannot simulate native Windows toast delivery or permission state.
    assert page.evaluate("() => data.settings.task_due_reminders_enabled === false")
    page.get_by_role("button", name="Settings", exact=True).click()
    assert page.locator("#task-reminders-toggle").is_disabled()
    assert not page.locator("#task-reminders-toggle").is_checked()
    assert "browser preview never sends task toasts" in page.locator(
        "#task-reminders-status").inner_text().lower()
    page.evaluate("""async () => {
      data = await previewInvoke('set_task_due_reminders', {enabled: true});
      render();
    }""")
    assert page.locator("#task-reminders-toggle").is_checked()
    assert page.locator("#task-reminders-toggle").is_disabled()
    page.evaluate("""async () => {
      data = await previewInvoke('set_task_due_reminders', {enabled: false});
      render();
    }""")
    page.get_by_role("button", name="Tasks", exact=True).click()
    page.locator("#todo-input").fill("Prepare launch")
    page.locator("#todo-priority").select_option("high")
    page.locator("#todo-due").fill("2027-06-14T09:30")
    page.get_by_role("button", name="Add task").click()
    page.get_by_text("Prepare launch", exact=True).wait_for()
    page.wait_for_function("""() => data.todos.some(task =>
      task.title === 'Prepare launch' && task.priority === 'high' &&
      task.due_at_ms > 1800000000000)""")
    assert page.get_by_text("HIGH · Due", exact=False).count() == 1
    first = page.evaluate("""() => {
      const todo = data.todos.find(item => item.title === 'Prepare launch');
      return {id: todo.id, created: todo.created_ms, done: todo.done};
    }""")
    page.get_by_role("button", name="Edit task: Prepare launch").click()
    assert page.locator("#todo-edit-form").is_visible()
    assert page.locator("#todo-edit-priority").input_value() == "high"
    assert page.locator("#todo-edit-due").input_value() == "2027-06-14T09:30"

    page.locator("#todo-edit-title").fill("Finalize launch")
    page.locator("#todo-edit-priority").select_option("low")
    page.locator("#todo-edit-due").fill("2027-06-16T11:00")
    page.get_by_role("button", name="Save task").click()
    page.get_by_text("Finalize launch", exact=True).wait_for()
    assert page.locator("#todo-edit-form").is_hidden()
    after = page.evaluate("""() => {
      const todo = data.todos.find(item => item.title === 'Finalize launch');
      return {id: todo.id, created: todo.created_ms, done: todo.done,
        priority: todo.priority, due: todo.due_at_ms};
    }""")
    assert after["id"] == first["id"] and after["created"] == first["created"]
    assert after["priority"] == "low" and after["due"] > 1800000000000
    page.get_by_role("button", name="Complete: Finalize launch").click()
    page.wait_for_function("() => data.todos.find(item => item.title === 'Finalize launch').done")
    page.evaluate("""async () => { data = await previewInvoke('load_data'); render(); }""")
    assert page.get_by_role("button", name="Mark incomplete: Finalize launch").count() == 1

    page.get_by_role("button", name="Edit task: Finalize launch").click()
    page.locator("#todo-edit-title").fill("Discard accidental input")
    page.get_by_role("button", name="Cancel edit").click()
    assert page.get_by_text("Discard accidental input").count() == 0
    assert page.get_by_text("Finalize launch", exact=True).count() == 1

    # A v0.17-style todo should still render and offer Normal/No due editor.
    page.evaluate("""() => {
      data.todos.push({id: 'old-task', title: 'Legacy task', done: false, created_ms: 1});
      localStorage.setItem(previewKey, JSON.stringify(data));
      render();
    }""")
    page.get_by_role("button", name="Edit task: Legacy task").click()
    assert page.locator("#todo-edit-priority").input_value() == "normal"
    assert page.locator("#todo-edit-due").input_value() == ""
    page.get_by_role("button", name="Cancel edit").click()
    page.screenshot(path=str(EVIDENCE / "browser-tasks-phase18-priority-due.png"))
    assert not errors, f"JS failures: {errors}"
    browser.close()

print("PASS: Chromium Phase18 task priorities, local due time, editing, cancel, persisted completion and legacy state; native Windows NOT TESTED")
