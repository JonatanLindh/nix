#!/usr/bin/env python3
"""List the inputs that differ between two flake.lock files.

usage: lock-diff.py <old flake.lock> <new flake.lock>

Prints them in the style of `nix flake update` ("• Updated input 'x': ..."),
for PR bodies when the update log itself is not available.
"""

import json
import sys
from datetime import datetime, timezone


def inputs(lock: dict) -> dict[str, dict]:
    """Map each input path ("hyprland/nixpkgs") to its locked attrs.

    Inputs that `follows` another one are left out, as nix does.
    """
    nodes, out = lock["nodes"], {}

    def walk(node: str, prefix: str, seen: frozenset) -> None:
        for name, target in nodes[node].get("inputs", {}).items():
            if not isinstance(target, str) or target in seen:
                continue
            path = f"{prefix}{name}"
            out[path] = nodes[target].get("locked", {})
            walk(target, f"{path}/", seen | {target})

    walk(lock["root"], "", frozenset({lock["root"]}))
    return out


def ref(locked: dict) -> str:
    t = locked.get("type", "?")
    if t in ("github", "gitlab", "sourcehut"):
        s = f"{t}:{locked.get('owner')}/{locked.get('repo')}/{locked.get('rev', '?')}"
    elif "url" in locked:
        s = f"{t}+{locked['url']}" if t not in ("tarball", "file") else locked["url"]
        if "rev" in locked:
            s += f"?rev={locked['rev']}"
    else:
        s = f"{t}:{locked.get('path', '?')}"
    if "lastModified" in locked:
        day = datetime.fromtimestamp(locked["lastModified"], timezone.utc).date()
        s = f"'{s}' ({day})"
    else:
        s = f"'{s}'"
    return s


def main(argv: list[str]) -> None:
    if len(argv) != 2:
        sys.exit(__doc__.split("\n\n")[1])
    old, new = (inputs(json.load(open(p))) for p in argv)
    for path in sorted(old.keys() | new.keys()):
        a, b = old.get(path), new.get(path)
        if a == b:
            continue
        if a is None:
            print(f"• Added input '{path}':\n    {ref(b)}")
        elif b is None:
            print(f"• Removed input '{path}'")
        else:
            print(f"• Updated input '{path}':\n    {ref(a)}\n  → {ref(b)}")


if __name__ == "__main__":
    main(sys.argv[1:])
