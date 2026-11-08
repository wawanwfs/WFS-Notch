"""WFS Notch Phase 22D: ambiguity-safe media controls, stale state and progress.

Synthetic browser/WinRT response tests. This is NOT interactive native Windows QA.
"""
from pathlib import Path
import re
from shutil import which
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
with sync_playwright() as pw:
    executable = which("chromium") or which("chromium-browser") or which("google-chrome")
    browser = pw.chromium.launch(
        **({"executable_path": executable} if executable else {}),
        headless=True, args=["--no-sandbox", "--disable-dev-shm-usage"],
    )
    page = browser.new_page(viewport={"width": 1024, "height": 768}, bypass_csp=True)
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    html = (ROOT / "src/index.html").read_text(encoding="utf-8")
    html = re.sub(r'<script\b[^>]*>\s*</script>', "", html)
    html = re.sub(r'<link rel="stylesheet"[^>]*>', "", html)
    page.set_content(html)
    page.evaluate("""() => document.querySelector('#panel-media').classList.add('active')""")
    page.add_script_tag(content="""const __testMedia = {
      calls: [],
      sessions: [
        {source:'browser.id', title:'Example browser', artist:'Tab', playback_status:'Playing',
          can_toggle:false, can_next:false, can_previous:false, ambiguous:true},
        {source:'spotify.id', title:'Local track', artist:'Artist', playback_status:'Paused',
          can_toggle:true, can_next:true, can_previous:true, ambiguous:false}
      ],
      current: {source:'browser.id', title:'Private tab song', artist:'Artist',
        artwork_data_url: 'data:image/png;base64,iVBORw0KGgo=', playback_status:'Playing',
        position_ms:5000, duration_ms:10000},
      selected: {source:'spotify.id', title:'Spotify track', artist:'Artist',
        playback_status:'Paused', position_ms:1000, duration_ms:20000},
      blocked: false
    };
    const nativeInvoke = async () => {};
    async function invoke(command, args) {
      __testMedia.calls.push({command, args});
      if (command === 'get_active_media_session') return __testMedia.current;
      if (command === 'list_media_sessions') return __testMedia.sessions;
      if (command === 'get_media_session_by_source') {
        if (__testMedia.blocked) return new Promise(resolve => { __testMedia.release = resolve });
        return __testMedia.selected;
      }
      if (command === 'control_media_session') return true;
      throw new Error('unexpected: ' + command);
    }
    function showError(error) { throw error; }
    """)
    page.add_script_tag(content=(ROOT / "src/features/media.js").read_text(encoding="utf-8"))
    page.wait_for_function("() => mediaDetailsInitialized")
    page.evaluate("() => refreshMediaDetails(true)")
    assert page.locator("#media-player-picker").input_value() == "spotify.id"
    assert page.locator('[data-selected-media="toggle"]').is_enabled()
    assert page.locator("#media-track-title").inner_text() == "Spotify track"
    page.locator('[data-selected-media="toggle"]').click()
    page.wait_for_function("() => __testMedia.calls.some(x => x.command === 'control_media_session')")
    assert page.evaluate("""() => __testMedia.calls.filter(x => x.command === 'control_media_session')
      .every(x => x.args.source === 'spotify.id')""")

    # A duplicate browser app ID must be listed but never allow controlling
    # the first tab just because its application ID matches.
    page.locator("#media-player-picker").select_option("browser.id")
    page.wait_for_function("""() => document.querySelector('#media-player-status').textContent
      .includes('Multiple tabs or players')""")
    assert page.locator('[data-selected-media="toggle"]').is_disabled()
    assert page.locator("#media-artwork").is_hidden()
    assert page.locator("#media-progress-bar").get_attribute("style") == "width: 0%;"
    assert page.evaluate("""() => __testMedia.calls.filter(x => x.command === 'control_media_session')
      .length""") == 1

    # An OS sleep/resume or long background pause must not extrapolate
    # the timestamp well beyond a recently observed playback position.
    page.evaluate("""() => {
      renderMediaDetails({source:'spotify.id',title:'Playing test',playback_status:'Playing',
        position_ms:5000,duration_ms:100000});
      mediaLastSampleAt = Date.now() - 30000;
      updateInterpolatedProgress();
    }""")
    assert page.locator("#media-position").inner_text() == "00:05"

    # In-flight requests made for a previously selected player cannot
    # overwrite the media card after the user changes selection.
    page.evaluate("""() => {
      __testMedia.sessions.push({source:'vlc.id',title:'VLC',artist:'',
        playback_status:'Playing',can_toggle:true,can_next:false,
        can_previous:false,ambiguous:false});
      __testMedia.selected = {source:'spotify.id',title:'Stale Spotify',
        playback_status:'Playing',position_ms:1,duration_ms:100};
      __testMedia.blocked = true;
      document.querySelector('#media-player-picker').value = 'spotify.id';
      __testMedia.pending = refreshMediaDetails(true);
    }""")
    page.wait_for_function("() => typeof __testMedia.release === 'function'")
    page.evaluate("""() => {
      document.querySelector('#media-player-picker').value = 'vlc.id';
      __testMedia.release(__testMedia.selected);
    }""")
    page.evaluate("() => __testMedia.pending")
    assert page.locator("#media-track-title").inner_text() != "Stale Spotify"
    assert not errors, f"Browser media JavaScript errors: {errors}"
    browser.close()
print("PASS: GSMTC duplicate-app ambiguity, control restrictions, stale selection and bounded progress (mock only, native Windows NOT TESTED)")
