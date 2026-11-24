#!/usr/bin/env bash
# Write the body of a flake-update PR. Shared by update-flake.yml (fresh
# update) and refresh-pr.yml (later pushes to the update branch).
#
# usage: pr-body.sh <out.md>
#
# Inputs, all through the environment (empty/unset is fine unless noted):
#   INTRO        first paragraph (required)
#   EVAL         outcome of the last host evaluation: success | failure
#   WARNINGS     true if that evaluation had warnings
#   FIXED        true if Claude fixed evaluation errors in a separate commit
#   RESOLVED     true if Claude acted on ##CHECK markers in a separate commit
#   NOTE         extra sentence appended to the status paragraph
#   CHECKS       outcome of checks.py: success | failure
#   BEFORE AFTER JSON files of versions (VERSIONS_EXPR), before and after
#   INPUTS       file with the input changes, shown in a code block
#   CHECKS_JSON  file from `checks.py --json`
#   EXTRA        markdown file inserted after the inputs (e.g. commit list)
# claude-summary.md and claude-checks-summary.md in the working directory,
# and eval-*.log from eval-hosts.sh, are included when present.
#
# Writes ok=true|false to $GITHUB_OUTPUT (and stdout when run elsewhere):
# whether both hosts evaluate.
set -uo pipefail

out=${1:?usage: pr-body.sh <out.md>}
: "${INTRO:?INTRO is required}"
EVAL=${EVAL:-failure} WARNINGS=${WARNINGS:-false} FIXED=${FIXED:-} RESOLVED=${RESOLVED:-}
NOTE=${NOTE:-} CHECKS=${CHECKS:-} BEFORE=${BEFORE:-} AFTER=${AFTER:-}
INPUTS=${INPUTS:-} CHECKS_JSON=${CHECKS_JSON:-} EXTRA=${EXTRA:-}

json_or_empty() { if [ -s "${1:-}" ]; then cat "$1"; else echo '{}'; fi; }

if [ "$EVAL" = success ]; then
  status="Both hosts evaluate and their Hyprland configs pass \`--verify-config\`."
  ok=true
else
  status="**Evaluation or the Hyprland config check fails.** Not safe to merge as is."
  ok=false
fi
if [ "$FIXED" = true ]; then
  status="The update caused evaluation errors or warnings, and Claude changed the config in a separate commit. Review that commit before merging. $status"
fi
if [ "$RESOLVED" = true ]; then
  status="$status Claude acted on passing \`##CHECK:\` markers in a separate commit; review that too."
fi
if [ "$ok" = true ] && [ "$WARNINGS" = true ]; then
  status="$status There are still evaluation warnings; see the logs below."
fi
[ -n "$NOTE" ] && status="$status $NOTE"

if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "ok=$ok" >>"$GITHUB_OUTPUT"
else
  echo "ok=$ok"
fi

{
  echo "$INTRO"
  echo
  echo "$status"
  echo
  echo "Nothing was built, so build-time failures are not covered."
  echo
  echo "### Versions"
  echo
  # Rows whose version changed are bold
  echo "| | before | after |"
  echo "|---|---|---|"
  jq -r -n --argjson b "$(json_or_empty "$BEFORE")" --argjson a "$(json_or_empty "$AFTER")" '
    ($b + $a | keys[]) as $k
    | ($b[$k] // "?") as $old
    | ($a[$k] // "?") as $new
    | if $old != $new
      then "| **\($k)** | \($old) | **\($new)** |"
      else "| \($k) | \($old) | \($new) |"
      end'
  echo
  echo "### Inputs"
  echo
  echo '```'
  if [ -s "$INPUTS" ]; then
    grep -v -E '^(warning|unpacking|copying|fetching)' "$INPUTS" || true
  else
    echo "(unknown)"
  fi
  echo '```'
  if [ -s "$EXTRA" ]; then
    echo
    cat "$EXTRA"
  fi
  if [ -s claude-summary.md ]; then
    echo
    echo "### Claude's summary"
    echo
    cat claude-summary.md
  fi
  echo
  echo "### Checks"
  echo
  if [ "$CHECKS" != success ]; then
    echo "**\`checks.py\` reported errors**; see the workflow logs."
    echo
  fi
  # True, unknown and broken markers; false ones are the normal state
  jq -r 'map(select(.result != "false"))
         | if length == 0 then "No `##CHECK:` marker is true."
           else .[] | "- **\(.error // .result)** `\(.file):\(.line)`: `\(.expr)`"
                + (if .result == "true" and .instructions == "" then " (no instructions; fix inferred)" else "" end)
           end' "$CHECKS_JSON" 2>/dev/null || echo "No check results."
  if [ -s claude-checks-summary.md ]; then
    echo
    echo "#### What Claude did"
    echo
    cat claude-checks-summary.md
  fi
  # Logs are only worth showing when something is wrong
  if [ "$ok" != true ] || [ "$WARNINGS" = true ]; then
    for log in eval-*.log; do
      [ -e "$log" ] || continue
      echo
      echo "### ${log%.log}"
      echo
      echo '```'
      grep -v -E '^(copying path|unpacking) ' "$log" | tail -n 60
      echo
      echo '```'
    done
  fi
} >"$out"
