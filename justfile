# List the available recipes
default:
    @just --list

# Evaluate both hosts without building (same check as CI); skipped if these contents already passed
check:
    @.github/scripts/checks.py --validate
    .github/scripts/eval-hosts.sh

# Evaluate both hosts even if these contents already passed
recheck:
    @.github/scripts/checks.py --validate
    .github/scripts/eval-hosts.sh --force

# Check, then push (jj does not run git hooks, so use this instead of `jj git push`)
push *args: check
    jj git push {{ args }}

# Link the Home Manager-generated .luarc.json into the repo for Hyprland Lua completions
luarc:
    ln -sfn ~/.config/hypr/.luarc.json .luarc.json

# Build and switch this host
switch:
    nh os switch

# Build and switch, offloading builds to the remote builders in /etc/nix/machines
switch-remote:
    nh os switch --builders "@/etc/nix/machines"

alias sw := switch
alias swr := switch-remote

# Report which ##CHECK: markers in the repo are true (see .github/scripts/checks.py)
checks *args:
    @.github/scripts/checks.py {{ args }}

# Check recipes ([group('check')]) are the functions of ##CHECK: markers.
# Exit 0 true, 1 false, else unknown. stdout reaches Claude in the update
# workflow: only own wording and identifiers there, never fetched free text
# (PR titles, commit messages, API errors); those go to stderr.
# See .github/scripts/checks.py.

# Check: nixpkgs PR has reached the nixos-unstable branch
[group('check')]
nixpkgs_unstable pr:
    @.github/scripts/check-nixpkgs.py branch nixos-unstable {{ pr }}

# Check: nixpkgs PR is in the nixpkgs revision pinned in flake.lock
[group('check')]
nixpkgs_locked pr:
    @.github/scripts/check-nixpkgs.py locked {{ pr }}

# Check: the linux-firmware in the locked nixpkgs ships a file (path under /lib/firmware)
[group('check')]
linux_firmware_has path:
    @.github/scripts/check-linux-firmware.py {{ quote(path) }}
