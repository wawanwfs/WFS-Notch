#!/usr/bin/env python3
"""Read-only Windows PE triage. No execution, unpacking, or copying of original code.
Run: python pe_triage.py binary1.exe binary2.exe --output report.json
"""
import argparse, hashlib, json, re, struct, subprocess
from pathlib import Path

SIGNALS = {
    'rust_compiler': b'rustc',
    'tauri_framework': b'tauri-2.',
    'tauri_invoke': b'__TAURI_INTERNALS__',
    'webview2': b'WebView2',
    'tauri_updater': b'tauri-plugin-updater-',
    'tokio': b'tokio-',
    'nsis': b'Nullsoft',
    'embedded_json': b'JSON',
}

def triage(filename: Path):
    data = filename.read_bytes()
    if len(data) < 512 or data[:2] != b'MZ':
        raise ValueError(f'Not an MZ executable: {filename}')
    pe_offset = struct.unpack_from('<I', data, 0x3c)[0]
    if data[pe_offset:pe_offset + 4] != b'PE\0\0':
        raise ValueError(f'Not a valid PE signature: {filename}')
    machine, section_count = struct.unpack_from('<HH', data, pe_offset + 4)
    optional_offset = pe_offset + 24
    magic = struct.unpack_from('<H', data, optional_offset)[0]
    optional_size = struct.unpack_from('<H', data, pe_offset + 20)[0]
    section_offset = optional_offset + optional_size
    sections = []
    for i in range(section_count):
        o = section_offset + i * 40
        name = data[o:o + 8].split(b'\0')[0].decode('ascii', errors='replace')
        raw_size, raw_offset = struct.unpack_from('<II', data, o + 16)
        sections.append({'name': name, 'disk_size': raw_size, 'offset': raw_offset})
    dlls = []
    try:
        command = subprocess.run(['objdump', '-p', str(filename)], capture_output=True, text=True, timeout=20)
        dlls = sorted(set(re.findall(r'DLL Name:\s*([^\s]+)', command.stdout)))
    except (FileNotFoundError, subprocess.TimeoutExpired):
        pass
    hits = {}
    lowered = data.lower()
    for key, needle in SIGNALS.items():
        location = lowered.find(needle.lower())
        hits[key] = {'present': location >= 0, 'first_file_offset': hex(location) if location >= 0 else None}
    version_regex = rb'tauri-([0-9]+\.[0-9]+\.[0-9]+)'
    match = re.search(version_regex, data)
    return {
        'filename': filename.name, 'size_bytes': len(data),
        'sha256': hashlib.sha256(data).hexdigest(),
        'architecture': {0x8664: 'x86_64', 0x14c: 'x86'}.get(machine, hex(machine)),
        'pe_format': 'PE32+' if magic == 0x20b else 'PE32',
        'sections': sections, 'import_dlls': dlls,
        'signals': hits,
        'visible_tauri_version': match.group(1).decode() if match else None,
        'execution': 'NOT EXECUTED',
    }

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('files', nargs='+', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    report = {'analysis_type': 'read-only static PE triage', 'results': [triage(f) for f in args.files]}
    blob = json.dumps(report, indent=2, ensure_ascii=False)
    if args.output: args.output.write_text(blob + '\n', encoding='utf-8')
    else: print(blob)
