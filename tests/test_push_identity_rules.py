"""Deterministic attribution guard regression: GitHub squash / merge semantics."""
from verify_push_identity import EMAIL, NAME, allowed_committer

owner = [NAME, EMAIL]
github = ['GitHub', 'noreply@github.com']
stranger = ['untrusted', 'untrusted@example.com']

assert allowed_committer(1, owner, "Owner-authored commit")
assert allowed_committer(2, github, "Merge pull request #14")
assert allowed_committer(1, github, "feat: WFS Notch Phase 12 (#14)")
assert not allowed_committer(1, github, "ordinary commit not merged")
assert not allowed_committer(1, github, "feat: WFS Notch (#x)")
assert not allowed_committer(0, github, "feat: WFS Notch (#14)")
assert not allowed_committer(3, github, "feat: WFS Notch (#14)")
assert not allowed_committer(1, stranger, "feat: WFS Notch (#14)")
print("PASS: owner identity and GitHub merge/squash-merge committer policy")
