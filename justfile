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
