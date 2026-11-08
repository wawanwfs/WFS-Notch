"""Fail CI if the active source tree regresses to a research handoff or broken asset paths."""
from pathlib import Path
from html.parser import HTMLParser
import re

root = Path(__file__).resolve().parents[1]
expected = [
    'src/index.html', 'src/app.js', 'src/features/media.js',
    'src/features/wellness.js', 'src/features/calendar.js',
    'src/styles/base.css', 'src-tauri/src/main.rs',
    'src-tauri/src/wellness.rs', 'docs/architecture.md',
    'docs/security.md', 'tests/static_audit.py',
]
for name in expected:
    assert (root / name).is_file(), f'Missing project file: {name}'
for legacy in ['00_INSTRUCTIONS', '01_ANALYSIS', '02_APP', '03_TOOLS',
               '04_TESTS', '05_BRANDING', '06_EVIDENCE',
               'src/parity.js', 'src-tauri/src/parity.rs']:
    assert not (root / legacy).exists(), f'Research scaffold leaked: {legacy}'
html = (root / 'src/index.html').read_text()
for path in re.findall(r'<(?:script|link)\b[^>]*(?:src|href)="([^"]+)"', html):
    if path.startswith(('http:', 'https:', 'data:')):
        continue
    assert (root / 'src' / path).is_file(), f'Broken HTML asset path: {path}'
assert not any(root.glob('src/*_plus.js'))
print('PASS: conventional app layout, original assets and valid local script/css references')
