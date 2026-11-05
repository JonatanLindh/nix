#!/usr/bin/env python3
"""Evaluate the ##CHECK: markers in this repository.

A marker is a comment line starting with `##CHECK:` followed by an
expression. The `##` comment lines directly below it are instructions for
what to do once the check is true; the update-flake workflow hands true
markers to Claude. Without instructions, Claude works out the fix from the
code the marker sits on (e.g. a nixpkgs input pinned for one PR and the
packages taken from it). For a marker that should only be reported, say so:
`## Report only, change nothing.`

  > ##CHECK: nixpkgs_locked(456789) && !nixpkgs_unstable(567890)
  > ## Drop the overrideAttrs below and patches/foo.patch.

Expressions: calls `name(arg, ...)`, `&&`, `||`, `!` and parentheses.
Arguments are numbers, bare words ([A-Za-z0-9_.:/+-]) or "double quoted"
strings. Functions are the justfile recipes in `[group('check')]`; a call
runs `just name arg...` and its exit status is the answer: 0 true, 1 false,
anything else unknown. Unknown propagates (false && ? is false, true && ? is
unknown), and each distinct call runs once.

Recipe output: stdout is the recipe's answer and is passed to Claude with
the true markers, so print only your own wording plus identifiers (PR
numbers, hashes, tags, paths). Never print fetched free text there (PR
titles or bodies, commit messages, API error text): anyone can write that,
and the Claude step has write access. Diagnostics go to stderr, which only
reaches logs and `just checks`. Each stream is cut to OUTPUT_LINES lines.

usage: checks.py             print every marker and its value
       checks.py --json      the same as JSON, for the workflow
       checks.py --validate  parse and check function names only; runs nothing

Markdown files are skipped, so docs can show example markers.

Exits 2 if a marker doesn't parse or calls an unknown function, otherwise 0.
"""

import json
import re
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GROUP = "check"
CALL_TIMEOUT = 120
# Recipe output kept per call
OUTPUT_LINES = 5
OUTPUT_WIDTH = 300

MARKER = re.compile(r"^\s*##CHECK:(.*)$")
INSTRUCTION = re.compile(r"^\s*##(?!CHECK:)(?: ?)(.*)$")

# Three-valued results
TRUE, FALSE, UNKNOWN = "true", "false", "unknown"


class ParseError(Exception):
    pass


# --- parsing -----------------------------------------------------------------

TOKEN = re.compile(
    r"""\s*(?:
        (?P<op>&&|\|\||!|\(|\)|,)
      | "(?P<str>(?:[^"\\]|\\.)*)"
      | (?P<word>[A-Za-z0-9_.:/+-]+)
    )""",
    re.VERBOSE,
)


def tokenize(src: str) -> list[tuple[str, str]]:
    toks, pos = [], 0
    src = src.rstrip()
    while pos < len(src):
        m = TOKEN.match(src, pos)
        if not m or m.end() == pos:
            raise ParseError(f"unexpected {src[pos:].strip()[:20]!r}")
        if m["op"] is not None:
            toks.append(("op", m["op"]))
        elif m["str"] is not None:
            toks.append(("str", re.sub(r"\\(.)", r"\1", m["str"])))
        else:
            toks.append(("word", m["word"]))
        pos = m.end()
        while pos < len(src) and src[pos].isspace():
            pos += 1
    return toks


# AST: ("call", name, (args...)) | ("not", e) | ("and", l, r) | ("or", l, r)
class Parser:
    def __init__(self, src: str):
        self.toks = tokenize(src)
        self.i = 0

    def peek(self):
        return self.toks[self.i] if self.i < len(self.toks) else (None, None)

    def take(self, kind=None, value=None):
        tok = self.peek()
        if tok[0] is None or (kind and tok[0] != kind) or (value and tok[1] != value):
            want = value or kind or "more input"
            got = "end of line" if tok[0] is None else repr(tok[1])
            raise ParseError(f"expected {want!r}, got {got}")
        self.i += 1
        return tok

    def parse(self):
        if not self.toks:
            raise ParseError("empty expression")
        e = self.or_()
        if self.peek()[0] is not None:
            raise ParseError(f"unexpected {self.peek()[1]!r}")
        return e

    def or_(self):
        e = self.and_()
        while self.peek() == ("op", "||"):
            self.take()
            e = ("or", e, self.and_())
        return e

    def and_(self):
        e = self.unary()
        while self.peek() == ("op", "&&"):
            self.take()
            e = ("and", e, self.unary())
        return e

    def unary(self):
        if self.peek() == ("op", "!"):
            self.take()
            return ("not", self.unary())
        if self.peek() == ("op", "("):
            self.take()
            e = self.or_()
            self.take("op", ")")
            return e
        name = self.take("word")[1]
        if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_-]*", name):
            raise ParseError(f"{name!r} is not a function name")
        self.take("op", "(")
        args = []
        if self.peek() != ("op", ")"):
            while True:
                kind, val = self.peek()
                if kind not in ("word", "str"):
                    raise ParseError(f"expected an argument, got {val!r}")
                args.append(self.take()[1])
                if self.peek() != ("op", ","):
                    break
                self.take()
        self.take("op", ")")
        return ("call", name, tuple(args))


def calls(e):
    if e[0] == "call":
        yield e
    else:
        for sub in e[1:]:
            yield from calls(sub)


# --- markers -----------------------------------------------------------------


@dataclass
class Marker:
    file: str
    line: int
    expr: str
    instructions: str
    ast: tuple | None = None
    error: str | None = None
    result: str | None = None
    calls: list["Call"] = field(default_factory=list)


def tracked_files() -> list[str]:
    # jj first: it snapshots the working copy, so new files count too
    for cmd in (["jj", "file", "list"], ["git", "ls-files"]):
        try:
            r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        except FileNotFoundError:
            continue
        if r.returncode == 0:
            return r.stdout.splitlines()
    sys.exit("checks: can't list files with jj or git")


def find_markers() -> list[Marker]:
    markers = []
    for rel in tracked_files():
        # Docs (CLAUDE.md etc.) show example markers
        if rel.endswith(".md"):
            continue
        try:
            lines = (ROOT / rel).read_text().splitlines()
        except (UnicodeDecodeError, IsADirectoryError, FileNotFoundError):
            continue
        for i, text in enumerate(lines):
            m = MARKER.match(text)
            if not m:
                continue
            instr = []
            for follow in lines[i + 1 :]:
                im = INSTRUCTION.match(follow)
                if not im:
                    break
                instr.append(im[1])
            markers.append(Marker(rel, i + 1, m[1].strip(), "\n".join(instr).strip()))
    return markers


# --- functions ---------------------------------------------------------------


def check_recipes() -> dict[str, tuple[int, float]]:
    """name -> (min args, max args) for the recipes in [group('check')]."""
    r = subprocess.run(
        ["just", "--dump", "--dump-format", "json"], cwd=ROOT, capture_output=True, text=True
    )
    if r.returncode != 0:
        sys.exit(f"checks: just --dump failed: {r.stderr.strip()}")
    out = {}
    for name, rec in json.loads(r.stdout)["recipes"].items():
        if {"group": GROUP} not in rec["attributes"]:
            continue
        params = rec["parameters"]
        lo = sum(1 for p in params if p["default"] is None and p["kind"] != "star")
        hi = float("inf") if any(p["kind"] != "singular" for p in params) else len(params)
        out[name] = (lo, hi)
    return out


def clip(text: str) -> str:
    """Non-empty lines, at most OUTPUT_LINES of OUTPUT_WIDTH chars."""
    lines = [l.rstrip()[:OUTPUT_WIDTH] for l in text.splitlines() if l.strip()]
    if len(lines) > OUTPUT_LINES:
        lines = lines[:OUTPUT_LINES] + ["…"]
    return "\n".join(lines)


@dataclass
class Call:
    call: str  # rendered, e.g. "nixpkgs_locked(123)"
    result: str
    stdout: str  # the recipe's answer; may reach Claude
    stderr: str  # diagnostics; logs only


class Evaluator:
    def __init__(self):
        self.cache: dict[tuple, Call] = {}

    def call(self, name: str, args: tuple) -> Call:
        key = (name, args)
        if key not in self.cache:
            try:
                r = subprocess.run(
                    ["just", name, *args],
                    cwd=ROOT,
                    capture_output=True,
                    text=True,
                    timeout=CALL_TIMEOUT,
                )
                code, stdout = r.returncode, r.stdout
                # Drop just's own "recipe failed" line; the recipe said why
                stderr = "\n".join(
                    l for l in r.stderr.splitlines() if not l.startswith("error: recipe ")
                )
            except subprocess.TimeoutExpired:
                code, stdout, stderr = None, "", f"timed out after {CALL_TIMEOUT}s"
            self.cache[key] = Call(
                f"{name}({', '.join(args)})",
                {0: TRUE, 1: FALSE}.get(code, UNKNOWN),
                clip(stdout),
                clip(stderr),
            )
        return self.cache[key]

    def eval(self, e, calls: list[Call]) -> str:
        op = e[0]
        if op == "call":
            c = self.call(e[1], e[2])
            calls.append(c)
            return c.result
        if op == "not":
            v = self.eval(e[1], calls)
            return {TRUE: FALSE, FALSE: TRUE}.get(v, UNKNOWN)
        # Short-circuit: the deciding value skips the right side
        stop = FALSE if op == "and" else TRUE
        left = self.eval(e[1], calls)
        if left == stop:
            return stop
        right = self.eval(e[2], calls)
        if right == stop:
            return stop
        return UNKNOWN if UNKNOWN in (left, right) else left


# --- main ----------------------------------------------------------------------


def main(argv: list[str]) -> int:
    if argv and argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    mode = argv[0] if argv else "--text"
    if mode not in ("--text", "--json", "--validate") or len(argv) > 1:
        print(__doc__, file=sys.stderr)
        return 2

    markers = find_markers()
    recipes = check_recipes()
    bad = False
    for m in markers:
        try:
            m.ast = Parser(m.expr).parse()
        except ParseError as err:
            m.error = f"parse error: {err}"
        else:
            for _, name, args in calls(m.ast):
                if name not in recipes:
                    m.error = f"unknown function {name}() (add a [group('{GROUP}')] recipe to the justfile)"
                    break
                lo, hi = recipes[name]
                if not lo <= len(args) <= hi:
                    want = lo if lo == hi else f"{lo}+" if hi == float("inf") else f"{lo}-{hi}"
                    m.error = f"{name}() takes {want} argument(s), got {len(args)}"
                    break
        if m.error:
            bad = True
            print(f"{m.file}:{m.line}: {m.error}\n    ##CHECK: {m.expr}", file=sys.stderr)

    if mode == "--validate":
        if not bad:
            print(f"{len(markers)} ##CHECK marker(s) ok")
        return 2 if bad else 0

    ev = Evaluator()
    for m in markers:
        if not m.error:
            m.result = ev.eval(m.ast, m.calls)

    if mode == "--json":
        json.dump(
            [
                {
                    "file": m.file,
                    "line": m.line,
                    "expr": m.expr,
                    "result": m.result,
                    "error": m.error,
                    "instructions": m.instructions,
                    # stdout may be shown to Claude, stderr only in logs
                    "calls": [asdict(c) for c in m.calls],
                }
                for m in markers
            ],
            sys.stdout,
            indent=2,
        )
        print()
    else:
        if not markers:
            print("no ##CHECK markers")
        for m in markers:
            if m.error:
                continue
            print(f"{m.result.upper():8} {m.file}:{m.line}  {m.expr}")
            for c in m.calls:
                print(f"         {c.call} = {c.result}")
                for line in c.stdout.splitlines():
                    print(f"           {line}")
                for line in c.stderr.splitlines():
                    print(f"           stderr: {line}")
            if m.result == TRUE and m.instructions:
                print("         to do: " + m.instructions.replace("\n", "\n                "))
    return 2 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
