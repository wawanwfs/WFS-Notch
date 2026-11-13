"""Exercise the deployed HTML/CSS/JS, not the native Windows compositor."""
from pathlib import Path
from shutil import which

from playwright.sync_api import expect, sync_playwright

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "test-results"
EVIDENCE.mkdir(exist_ok=True)


with sync_playwright() as pw:
    executable = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(
        **({"executable_path": executable} if executable else {}),
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"],
    )
    errors = []
    for width, height in [(760, 470), (560, 420), (360, 320)]:
        page = browser.new_page(viewport={"width": width, "height": height}, reduced_motion="reduce")
        page.on("pageerror", lambda error: errors.append(str(error)))
        page.goto((ROOT / "src" / "index.html").as_uri())
        expanded = page.locator("#expanded")
        dialog = page.get_by_role("dialog", name="Find a module")
        finder = page.get_by_role("button", name="Find module", exact=True)
        search = dialog.get_by_role("searchbox", name="Search modules")
        options = dialog.locator(".palette-option")
        status = dialog.get_by_role("status")

        finder.click()
        expect(search).to_be_focused()
        expect(options).to_have_count(16)
        expect(status).to_have_text("16 modules available")
        assert page.locator("main").evaluate("node => node.inert")
        assert page.locator(".tabs").evaluate("node => node.inert")
        bounds = dialog.locator(".palette-dialog").bounding_box()
        assert bounds and 0 <= bounds["x"] < width and bounds["x"] + bounds["width"] <= width
        assert 0 <= bounds["y"] < height and bounds["y"] + bounds["height"] <= height
        page.screenshot(path=str(EVIDENCE / f"browser-module-finder-{width}x{height}.png"))

        page.keyboard.press("Shift+Tab")
        expect(dialog.get_by_role("button", name="Close module finder")).to_be_focused()
        page.keyboard.press("Shift+Tab")
        expect(options.last).to_be_focused()
        page.keyboard.press("Tab")
        expect(dialog.get_by_role("button", name="Close module finder")).to_be_focused()
        page.keyboard.press("Escape")
        expect(dialog).to_be_hidden()
        expect(expanded).to_be_visible()
        expect(finder).to_be_focused()
        assert not page.locator("main").evaluate("node => node.inert")

        page.keyboard.press("Control+k")
        search.fill("zzz none")
        expect(options).to_have_count(0)
        expect(status).to_have_text("No matching module. Try another search.")
        page.keyboard.press("Enter")
        expect(dialog).to_be_visible()
        search.fill("POMODORO session")
        expect(options).to_have_count(1)
        page.keyboard.press("ArrowDown")
        expect(options.first).to_be_focused()
        page.keyboard.press("Enter")
        expect(dialog).to_be_hidden()
        expect(page.locator("#panel-timer")).to_be_visible()
        expect(page.locator("#nav-timer")).to_be_focused()

        page.keyboard.press("Escape")
        collapsed = page.locator("#collapsed")
        expect(collapsed).to_be_focused()
        page.keyboard.press("Enter")
        expect(page.locator("#nav-timer")).to_be_focused()
        page.keyboard.press("Escape")
        expect(collapsed).to_be_focused()
        page.keyboard.press("Control+k")
        expect(search).to_be_focused()
        page.keyboard.press("Escape")
        expect(finder).to_be_focused()

        # Exercise all module routes through the same finder and focus contract.
        if width == 760:
            for tab in page.locator(".tab").all():
                module = tab.get_attribute("data-tab")
                finder.click()
                search.fill(tab.text_content())
                dialog.get_by_role("button", name=tab.text_content(), exact=False).first.click()
                expect(dialog).to_be_hidden()
                expect(page.locator(f"#panel-{module}")).to_be_visible()
                expect(tab).to_be_focused()

            page.locator("#nav-notes").click()
            page.locator("#note-new").click()
            page.locator("#note-body").fill("Keep my draft")
            # Deterministic in-flight save stand-in to exercise modal cancellation.
            page.evaluate("""() => {
              clearTimeout(noteSaveTimeout);
              flushNote = () => new Promise(resolve => { window.finishTestSave = resolve; });
            }""")
            finder.click()
            search.fill("Calendar")
            page.keyboard.press("Enter")
            page.wait_for_function("typeof window.finishTestSave === 'function'")
            page.keyboard.press("Escape")
            page.evaluate("window.finishTestSave(true)")
            expect(page.locator("#panel-notes")).to_be_visible()
            expect(page.locator("#note-body")).to_have_value("Keep my draft")
            page.evaluate("flushNote = async () => false")
            finder.click()
            search.fill("Calendar")
            page.keyboard.press("Enter")
            expect(status).to_contain_text("Module not opened")
            page.keyboard.press("Escape")
            expect(page.locator("#panel-notes")).to_be_visible()
        page.close()
    assert not errors, errors
    browser.close()
print("PASS: 16 module routes, three viewport sizes, modal focus, keyboard and pending Notes guards")
