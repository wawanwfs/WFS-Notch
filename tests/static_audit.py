"""Dependency-free structural audit for the WFS Notch product source tree."""
from pathlib import Path
import json
import tomllib
import re
import xml.etree.ElementTree as ET
from html.parser import HTMLParser

root = Path(__file__).resolve().parents[1]
app = root
html = (app / 'src/index.html').read_text()
js = '\n'.join((app / 'src' / filename).read_text() for filename in ('app.js', 'modules.js', 'parity.js', 'calendar_plus.js', 'system_details.js', 'media_sessions.js', 'ux_polish.js', 'display_ui.js'))
rust = (app / 'src-tauri/src/main.rs').read_text()
all_rust = '\n'.join((app / 'src-tauri/src' / filename).read_text() for filename in ('main.rs', 'extra.rs', 'parity.rs', 'calendar_ics.rs', 'system_details.rs', 'media_sessions.rs', 'display.rs', 'notifications.rs'))
config = json.loads((app / 'src-tauri/tauri.conf.json').read_text())
capabilities = json.loads((app / 'src-tauri/capabilities/default.json').read_text())

class IdParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids = []
    def handle_starttag(self, tag, attrs):
        self.ids.extend(value for key, value in attrs if key == 'id')

parser = IdParser(); parser.feed(html)
ids = parser.ids
assert len(ids) == len(set(ids)), 'Duplicate HTML ids'
referenced_ids = set(re.findall(r"\$\('#([A-Za-z0-9_-]+)'\)", js))
assert referenced_ids.issubset(set(ids)), f'Missing elements: {referenced_ids - set(ids)}'
commands = set(re.findall(r'\#\[tauri::command\]\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)', all_rust))
frontend_calls = set(re.findall(r"(?:mutation|invoke)\('([a-z_]+)'", js))
extra_commands = set(re.findall(r'\#\[tauri::command\]\s*pub fn\s+(\w+)', (app / 'src-tauri/src/extra.rs').read_text()))
assert frontend_calls.issubset(commands | extra_commands), f'Missing Rust command(s): {frontend_calls - (commands | extra_commands)}'
assert config['productName'] == 'WFS Notch'
assert config['version'] == tomllib.loads((app / 'src-tauri/Cargo.toml').read_text())['package']['version']
assert config['identifier'] == 'id.wfs.notch'
assert capabilities['windows'] == ['main']
assert config['app']['withGlobalTauri'] is True
for path in config['bundle']['icon']:
    assert (app / 'src-tauri' / path).exists(), path
for path in (root / 'assets' / 'branding').glob('*.svg'):
    ET.parse(path)
assert (app / 'src/assets/wfs-notch-mark.svg').exists()
assert 'update_event' in commands and 'update_event' in js
assert len(commands) >= 20
extra = (app / 'src-tauri/src/extra.rs').read_text()
extra_commands = set(re.findall(r'\#\[tauri::command\]\s*pub fn\s+(\w+)', extra))
assert {'add_habit', 'open_quick_link', 'media_key'} <= extra_commands
assert {'import_ics', 'export_ics', 'get_active_media_session', 'get_display_options', 'set_display_preference', 'set_notification_preferences', 'notify_due_event', 'notify_test', 'list_media_sessions', 'control_media_session', 'set_notch_on_top'} <= commands
assert 'calendar_ics::import_ics' in rust and 'calendar_ics::export_ics' in rust
print('PASS: static audit |', len(ids), 'unique element IDs |', len(frontend_calls), 'bound frontend commands |', len(commands), 'Rust commands | JSON/SVG/icons checked')
