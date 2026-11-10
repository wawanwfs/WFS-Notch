"""Phase30A display-selection UI regression with a deterministic native adapter mock.

This checks only the UI's truthful disconnected monitor state. Physical Windows
monitor replug, OS window relocation, fullscreen and RDP require manual QA.
"""
from pathlib import Path
from shutil import which
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
script = (ROOT / "src" / "features" / "displays.js").read_text(encoding="utf-8")
html = """
<button class="tab" data-tab="settings" type="button">Settings</button>
<button id="display-refresh" type="button">Refresh</button>
<button id="display-apply" type="button">Apply</button>
<select id="display-monitor" aria-label="Monitor"></select>
<input id="display-top-gap" type="number" value="12">
<p id="display-status" role="status"></p>
"""

with sync_playwright() as pw:
    exe = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(
        **({"executable_path": exe} if exe else {}),
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"],
    )
    page = browser.new_page()
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.set_content(html)
    page.add_script_tag(content="""'use strict';
      const nativeInvoke = async () => {};
      let writeQueue = Promise.resolve();
      let data = {settings:{}};
      let mockSnapshot = {
        displays:[{id:'name:DISPLAY1',name:'DISPLAY1',width:1920,
          height:1080,scale_factor:1.25,is_primary:true}],
        monitor_id:'name:DISPLAY2',margin_px:12,active_monitor_id:'',
        using_fallback:false,selected_monitor_unavailable:true
      };
      let applied = null;
      async function invoke(command, args) {
        if (command === 'get_display_options') return mockSnapshot;
        if (command === 'set_display_preference') {
          applied = args;
          mockSnapshot.monitor_id = args.monitorId;
          mockSnapshot.selected_monitor_unavailable = false;
          mockSnapshot.active_monitor_id = 'name:DISPLAY1';
          return {settings:{monitor_id:args.monitorId}};
        }
        throw Error('unexpected command ' + command);
      }
      async function flushNote() { return true; }
      function render() {}
      function showInfo() {}
      function showError(error) { throw Error(String(error)); }
    """)
    page.add_script_tag(content=script)
    page.wait_for_function("""() => document.querySelector('#display-status')
      .textContent.includes('monitor(s) detected')""")
    status = page.locator("#display-status").inner_text()
    assert "Saved display disconnected" in status
    assert "not intentionally move" in status
    assert "Always-on-top is disabled" in status
    assert not page.locator("#display-refresh").is_disabled()
    assert page.locator("#display-monitor").input_value() == "name:DISPLAY2"
    assert "position retained" in page.locator(
        '#display-monitor option[value="name:DISPLAY2"]').inner_text()
    page.locator("#display-monitor").select_option("primary")
    page.locator("#display-apply").click()
    page.wait_for_function("""() => applied?.monitorId === 'primary' &&
      document.querySelector('#display-monitor').value === 'primary'""")
    assert "Saved display disconnected" not in page.locator("#display-status").inner_text()
    assert page.evaluate("applied.marginPx") == 12
    assert not errors, f"JavaScript errors: {errors}"
    browser.close()
print("PASS: pinned-monitor disconnect retains preference; explicit Primary selection updates status")
