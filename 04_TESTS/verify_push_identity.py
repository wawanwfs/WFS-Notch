"""Audit the GitHub push actor and author metadata for the main branch.

This is *not* cryptographic identity proof. Enforce branch rules + signed commits
on GitHub for stronger guarantees. Doesn't run for pull_request jobs.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

OWNER = 'wawanwfs'
EMAIL = '90094394+wawanwfs@users.noreply.github.com'
NAME = 'WAHYU FAJAR SETIAWAN'


def git(*arguments: str) -> str:
    return subprocess.check_output(['git', *arguments], text=True).strip()


def main() -> int:
    actor = os.environ.get('GITHUB_ACTOR', '').lower()
    if actor != OWNER:
        print(f'FAIL: push actor {actor!r} is not @{OWNER}')
        return 1
    event_path = os.environ.get('GITHUB_EVENT_PATH')
    event = json.loads(Path(event_path).read_text()) if event_path and Path(event_path).exists() else {}
    before = event.get('before', '')
    after = event.get('after') or git('rev-parse', 'HEAD')
    if before and set(before) != {'0'}:
        try:
            git('cat-file', '-e', f'{before}^{{commit}}')
            revision_range = f'{before}..{after}'
        except subprocess.CalledProcessError:
            # Explicitly fail rather than silently ignoring unknown history.
            print(f'FAIL: prior commit {before} not present. Fetch complete history.')
            return 1
    else:
        revision_range = after
    hashes = git('rev-list', revision_range).splitlines()
    if not hashes:
        print('PASS: no new commits')
        return 0
    for sha in hashes:
        # Field separation preserves spaces in the author's display name.
        author = git('show', '-s', '--format=%an%x00%ae', sha).split('\x00')
        committer = git('show', '-s', '--format=%cn%x00%ce', sha).split('\x00')
        if author != [NAME, EMAIL] or committer != [NAME, EMAIL]:
            print(f'FAIL: commit {sha} author={author} committer={committer}')
            return 1
    print(f'PASS: @{OWNER} pushed {len(hashes)} commit(s) with configured author/committer metadata')
    return 0


if __name__ == '__main__':
    sys.exit(main())
