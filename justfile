# List the available recipes
default:
    @just --list

# Evaluate both hosts without building (same check as CI); skipped if these contents already passed
check:
    .github/scripts/eval-hosts.sh

# Evaluate both hosts even if these contents already passed
recheck:
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
