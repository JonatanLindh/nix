#!/usr/bin/env bash
# Check a host's generated Hyprland config with that host's Hyprland
# (`Hyprland --verify-config`), so a Hyprland update that renames or removes
# something the Lua config uses is caught before switching. Needs no display,
# GPU or session. Usage: verify-hyprland.sh <host>
#
# Exit 0: config ok. 1: config errors (printed). Anything else: the check
# itself could not run.
#
# Must not run as root: when it can write to /, Hyprland 0.57's verify mode
# opens its IPC socket there and segfaults.
set -uo pipefail

host=${1:?usage: verify-hyprland.sh <host>}
here=$(dirname "$(readlink -f "$0")")

if ! out=$(nix build --no-link --print-out-paths --impure --expr \
  "import $here/hyprland-config.nix { flake = \"$PWD\"; host = \"$host\"; }"); then
  echo "$host: could not build the Hyprland config"
  exit 2
fi

# Copies, not the store symlinks: Hyprland resolves the config path and
# looks for require()d files next to the resolved file.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/config" "$tmp/runtime"
cp -rL --no-preserve=mode "$out/hypr" "$tmp/config/hypr"

timeout 60 env -i HOME="$tmp" XDG_CONFIG_HOME="$tmp/config" XDG_RUNTIME_DIR="$tmp/runtime" \
  "$out/hyprland/bin/Hyprland" --verify-config -c "$tmp/config/hypr/hyprland.lua" \
  >"$tmp/log" 2>&1
status=$?

# Only the result section; the rest is startup noise
result=$(sed -n '/Config parsing result/,$p' "$tmp/log" | sed '1d' | sed '/^$/d' |
  sed "s|$tmp/config/|~/.config/|g")

case $status in
0) echo "$host: Hyprland config ok" ;;
1)
  echo "$host: Hyprland config has errors"
  echo "$result"
  echo "(~/.config/hypr/general.lua and lib.lua come from modules/home/graphical/hyprland/lua/," \
    "hyprland.lua is generated from modules/home/graphical/hyprland/default.nix and host files." \
    "API stubs: $(readlink -f "$out/hyprland")/share/hypr/stubs, example config: .../share/hypr/hyprland.lua)"
  ;;
*)
  echo "$host: Hyprland --verify-config did not finish (exit $status)"
  tail -n 20 "$tmp/log"
  status=2
  ;;
esac
exit "$status"
