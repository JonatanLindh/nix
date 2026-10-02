#!/usr/bin/env bash
# Evaluate every host's system derivation without building it.
# Writes the output per host to eval-<host>.log and fails if any host fails.
# Evaluation warnings do not fail the script; they are reported as the
# `warnings` step output when run in GitHub Actions.
set -uo pipefail

hosts=(desktop xps)
status=0

for host in "${hosts[@]}"; do
  echo "::group::Evaluate $host"
  if nix eval --raw ".#nixosConfigurations.$host.config.system.build.toplevel.drvPath" \
    >"eval-$host.log" 2>&1; then
    echo "ok: $(tail -n 1 "eval-$host.log")"
  else
    echo "FAILED"
    status=1
  fi
  cat "eval-$host.log"
  echo
  echo "::endgroup::"
done

warnings=false
if grep -q -E '^(evaluation warning|trace: (evaluation )?warning):' eval-*.log; then
  warnings=true
  echo "Evaluation warnings found"
fi
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "warnings=$warnings" >>"$GITHUB_OUTPUT"
fi

exit "$status"
