# ##CHECK: markers

This repo marks temporary workarounds with `##CHECK:` comments, so they get
removed once upstream catches up. The weekly update workflow evaluates them
and has Claude act on the ones that became true. Full reference:
`.github/scripts/checks.py` (docstring).

```nix
##CHECK: nixpkgs_locked(456789)
## Optional instructions for when the check is true: what to remove or
## change. Without them, the fix is inferred from the code below.
foo = prev.foo.overrideAttrs (o: { patches = o.patches ++ [ ./foo.patch ]; });
```

## When changing this repo

- **Adding a workaround for something upstream will fix** (pinned or extra
  nixpkgs input, overlay/override, backported patch, package taken from
  elsewhere, extra firmware, disabled test, option set only to dodge a bug):
  put a `##CHECK:` marker directly above it that becomes true when the fix
  lands. Add `##` instructions when the cleanup isn't obvious from the code
  under the marker, e.g. when it spans several files.
- **Existing markers:** leave them alone unless you are resolving that
  marker. When you remove a workaround, remove its marker; when you move
  one, move the marker with it.
- **Functions** are the justfile recipes in `[group('check')]`; list them
  with `just --list`. If none fits, add one (see below) rather than writing
  a marker that can never become true.
- **Verify** with `just checks --validate` (syntax, function names,
  argument counts; offline) and `just checks` (evaluates everything).
  `just check` runs the validation too.

## Syntax

- Calls `name(arg, ...)`; args are numbers, bare words
  (`[A-Za-z0-9_.:/+-]`) or `"quoted strings"`.
- `&&`, `||`, `!`, parentheses. Results are true / false / unknown;
  unknown (network error etc.) never counts as true.
- The `##` lines directly below the marker are its instructions; the first
  line that isn't a `##` comment ends them. A marker that should only be
  reported says so: `## Report only, change nothing.`
- The marker must start its line (after indentation). Markdown files are
  not scanned, so examples there are safe; elsewhere, don't start an
  example line with `##CHECK:` or it becomes a real marker.

## Adding a check function

A justfile recipe in `[group('check')]`, usually calling a script in
`.github/scripts/`:

- Exit 0 true, 1 false, anything else unknown. Failures (network, rate
  limits, bad arguments) must be unknown, never false.
- **stdout is passed to Claude** in the update workflow, which has write
  access. Print only your own wording plus identifiers (PR numbers, hashes,
  tags, paths). Never print fetched free text (PR titles or bodies, commit
  messages, API error text) to stdout; that goes to stderr, which only
  reaches logs.
- Be fast and avoid rate-limited anonymous APIs: `gh api` is authenticated
  (GH_TOKEN in CI); for linux-firmware use git.kernel.org, not GitLab.
- Test it on a case that is true, one that is false and one that fails.
- Python helpers have unit tests: `python3 -I .github/scripts/test_checks.py`.
