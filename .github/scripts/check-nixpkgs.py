#!/usr/bin/env python3
"""Has a nixpkgs pull request reached a branch, or the pinned revision?

usage: check-nixpkgs.py branch <branch> <pr>
       check-nixpkgs.py locked <pr>      # rev of the root nixpkgs input in flake.lock

Exit status follows the ##CHECK: convention (see checks.py):
  0  yes: the PR is merged and its merge commit is in the target
  1  no: not merged yet, closed unmerged, or not in the target yet
  2  unknown: API or network failure, bad arguments
Talks to GitHub through `gh api`, so it uses gh's login (GH_TOKEN in CI).
"""

import json
import subprocess
import sys
from pathlib import Path

REPO = "NixOS/nixpkgs"
YES, NO, UNKNOWN = 0, 1, 2


def fail(msg: str) -> "NoReturn":
    print(f"check-nixpkgs: {msg}", file=sys.stderr)
    sys.exit(UNKNOWN)


def gh(path: str) -> dict:
    r = subprocess.run(["gh", "api", path], capture_output=True, text=True)
    if r.returncode != 0:
        fail(f"gh api {path}: {r.stderr.strip()}")
    return json.loads(r.stdout)


def locked_rev() -> str:
    lock = json.loads((Path(__file__).resolve().parents[2] / "flake.lock").read_text())
    nodes = lock["nodes"]
    name = nodes[lock["root"]]["inputs"]["nixpkgs"]
    if not isinstance(name, str):  # follows another input
        fail("root nixpkgs input is a follows; can't tell which rev it is")
    return nodes[name]["locked"]["rev"]


def main(argv: list[str]) -> None:
    if len(argv) == 3 and argv[0] == "branch":
        target, label, pr = argv[1], argv[1], argv[2]
    elif len(argv) == 2 and argv[0] == "locked":
        target = locked_rev()
        label, pr = f"flake.lock ({target[:12]})", argv[1]
    else:
        fail(__doc__.split("\n\n")[1])
    if not pr.isdigit():
        fail(f"PR must be a number, got {pr!r}")

    p = gh(f"repos/{REPO}/pulls/{pr}")
    if not p.get("merged_at"):
        state = "closed without merging" if p["state"] == "closed" else "not merged yet"
        print(f"#{pr} is {state}")
        sys.exit(NO)

    sha = p["merge_commit_sha"]
    # "ahead"/"identical": target contains sha. per_page=1 keeps the reply small.
    status = gh(f"repos/{REPO}/compare/{sha}...{target}?per_page=1")["status"]
    if status in ("ahead", "identical"):
        print(f"#{pr} ({sha[:12]}) is in {label}")
        sys.exit(YES)
    if status in ("behind", "diverged"):
        print(f"#{pr} ({sha[:12]}) is merged but not in {label} yet")
        sys.exit(NO)
    fail(f"unexpected compare status {status!r}")


if __name__ == "__main__":
    main(sys.argv[1:])
