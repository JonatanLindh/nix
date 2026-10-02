#!/usr/bin/env bash
# Evaluate every host's system derivation without building it.
# Writes the output per host to eval-<host>.log and fails if any host fails.
# Evaluation warnings do not fail the script; they are reported as the
# `warnings` step output when run in GitHub Actions.
#
# Locally, a passing result is remembered per working-copy tree in
# .eval-checked (gitignored), so the same contents are not evaluated twice.
# Pass --force to evaluate anyway.
set -uo pipefail

hosts=(desktop xps)
status=0
ci=${GITHUB_ACTIONS:-}
cache=.eval-checked
force=false
[ "${1:-}" = --force ] && force=true

# Fingerprint of the working copy's contents; unaffected by commit messages
tree=""
if [ -z "$ci" ] && commit=$(jj log -r @ --no-graph -T commit_id 2>/dev/null); then
  tree=$(git rev-parse "$commit^{tree}" 2>/dev/null) || tree=""
fi

if [ -n "$tree" ] && [ "$force" = false ] && hit=$(grep -m 1 "^$tree " "$cache" 2>/dev/null); then
  echo "already checked: these contents evaluate (${tree:0:12})"
  [ "${hit#* }" = true ] && echo "note: that check had evaluation warnings; use --force to see them"
  exit 0
fi

for host in "${hosts[@]}"; do
  [ -n "$ci" ] && echo "::group::Evaluate $host"
  if nix eval --raw ".#nixosConfigurations.$host.config.system.build.toplevel.drvPath" \
    >"eval-$host.log" 2>&1; then
    echo "$host: ok"
    # Locally the log is only interesting when something is wrong
    [ -n "$ci" ] && cat "eval-$host.log" && echo
  else
    echo "$host: FAILED"
    status=1
    cat "eval-$host.log"
    echo
  fi
  [ -n "$ci" ] && echo "::endgroup::"
done

warnings=false
if grep -E '^(evaluation warning|trace: (evaluation )?warning):' eval-*.log; then
  warnings=true
  echo "Evaluation warnings found"
fi
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "warnings=$warnings" >>"$GITHUB_OUTPUT"
fi

if [ "$status" -eq 0 ] && [ -n "$tree" ]; then
  # Keep the most recent 50 passing trees
  { echo "$tree $warnings"; grep -v "^$tree " "$cache" 2>/dev/null | head -n 49; } >"$cache.tmp"
  mv "$cache.tmp" "$cache"
fi

exit "$status"
