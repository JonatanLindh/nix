#!/usr/bin/env python3
"""Does the linux-firmware in the locked nixpkgs ship a given file?

usage: check-linux-firmware.py <path under /lib/firmware>
  e.g. check-linux-firmware.py mediatek/mt7927/BT_RAM_CODE_MT6639_2_1_hdr.bin

Evaluates which upstream tag nixpkgs builds linux-firmware from (no download),
then asks git.kernel.org whether that tag has the file (a HEAD request, no
content).

Exit status follows the ##CHECK: convention (see checks.py):
  0  yes, 1 no, 2 unknown (eval, network or rate-limit failure)
"""

import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CGIT = "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git"
# Both hosts share nixpkgs; any of them will do
ATTR = ".#nixosConfigurations.desktop.pkgs.linux-firmware.src"
YES, NO, UNKNOWN = 0, 1, 2
# kernel.org refuses urllib's default User-Agent with 403
HEADERS = {"User-Agent": "nixos-config-checks/1 (check-linux-firmware.py)"}


def fail(msg: str):
    print(f"check-linux-firmware: {msg}", file=sys.stderr)
    sys.exit(UNKNOWN)


def locked_tag() -> str:
    r = subprocess.run(
        ["nix", "eval", "--raw", ATTR, "--apply", "s: s.tag or s.rev"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        fail(f"nix eval {ATTR}: {r.stderr.strip().splitlines()[-1:]}")
    return r.stdout.strip()


def exists(path: str, ref: str) -> bool:
    # kernel.org's cgit, not GitLab's API: GitLab rate-limits anonymous
    # callers per IP for tens of minutes, cgit answers a HEAD in ~0.1s.
    url = f"{CGIT}/plain/{urllib.parse.quote(path)}?h={urllib.parse.quote(ref, safe='')}"
    try:
        with urllib.request.urlopen(urllib.request.Request(url, method="HEAD", headers=HEADERS), timeout=20):
            return True
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return False
        fail(f"git.kernel.org answered {e.code} for {path} at {ref}")
    except (urllib.error.URLError, TimeoutError) as e:
        fail(f"git.kernel.org unreachable: {e}")


def main(argv: list[str]) -> None:
    if len(argv) != 1 or not argv[0].strip("/"):
        fail(__doc__.split("\n\n")[1])
    path = argv[0].strip("/")
    tag = locked_tag()
    if exists(path, tag):
        print(f"linux-firmware {tag} ships {path}")
        sys.exit(YES)
    print(f"linux-firmware {tag} does not ship {path}")
    sys.exit(NO)


if __name__ == "__main__":
    main(sys.argv[1:])
