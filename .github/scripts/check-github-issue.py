#!/usr/bin/env python3
"""Has a GitHub issue been closed as completed?

usage: check-github-issue.py <owner/repo> <number>

Exit status follows the ##CHECK: convention (see checks.py):
  0  yes: the issue is closed as completed
  1  no: the issue is still open
  2  unknown: closed for another reason (not planned, duplicate), API or
     network failure, bad arguments
Talks to GitHub through `gh api`, so it uses gh's login (GH_TOKEN in CI).
Only the state and state reason are read; the title and body never are.
"""

import json
import re
import subprocess
import sys

YES, NO, UNKNOWN = 0, 1, 2


def fail(msg: str) -> "NoReturn":
    print(f"check-github-issue: {msg}", file=sys.stderr)
    sys.exit(UNKNOWN)


def main(argv: list[str]) -> None:
    if len(argv) != 2:
        fail(__doc__.split("\n\n")[1])
    repo, num = argv
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        fail(f"repo must be owner/name, got {repo!r}")
    if not num.isdigit():
        fail(f"issue must be a number, got {num!r}")

    path = f"repos/{repo}/issues/{num}"
    r = subprocess.run(
        ["gh", "api", path, "--jq", "{state, state_reason}"],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        fail(f"gh api {path}: {r.stderr.strip()}")
    issue = json.loads(r.stdout)
    state, reason = issue.get("state"), issue.get("state_reason")

    if state == "open":
        print(f"{repo}#{num} is still open")
        sys.exit(NO)
    if state == "closed" and reason == "completed":
        print(f"{repo}#{num} is closed as completed")
        sys.exit(YES)
    if state == "closed" and reason in ("not_planned", "duplicate"):
        # Our own wording: the fix may be tracked elsewhere, so a person decides.
        print(f"{repo}#{num} is closed as {reason}; check where it is tracked now")
        sys.exit(UNKNOWN)
    fail(f"unexpected state {state!r} / reason {reason!r}")


if __name__ == "__main__":
    main(sys.argv[1:])
