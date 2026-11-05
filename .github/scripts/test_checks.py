#!/usr/bin/env python3
"""Tests for checks.py's parser and three-valued evaluator. No network.

run: python3 -I .github/scripts/test_checks.py
"""

import sys

sys.dont_write_bytecode = True
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import checks  # noqa: E402
from checks import FALSE, TRUE, UNKNOWN, ParseError, Parser  # noqa: E402


class FakeEvaluator(checks.Evaluator):
    """f(true) / f(false) / f(unknown) return their argument; records calls."""

    def __init__(self):
        super().__init__()
        self.ran = []

    def call(self, name, args):
        self.ran.append((name, args))
        return checks.Call(f"{name}({', '.join(args)})", args[0], "", "")


def ev(src):
    e = FakeEvaluator()
    return e.eval(Parser(src).parse(), []), e.ran


class ParseTest(unittest.TestCase):
    def test_call_args(self):
        self.assertEqual(
            Parser('f(1, foo-bar, "a b\\" c", x.y/z:1)').parse(),
            ("call", "f", ("1", "foo-bar", 'a b" c', "x.y/z:1")),
        )
        self.assertEqual(Parser("f()").parse(), ("call", "f", ()))

    def test_precedence(self):
        # && binds tighter than ||, ! tighter than &&
        self.assertEqual(
            Parser("!a() || b() && c()").parse(),
            ("or", ("not", ("call", "a", ())), ("and", ("call", "b", ()), ("call", "c", ()))),
        )
        self.assertEqual(
            Parser("(a() || b()) && c()").parse(),
            ("and", ("or", ("call", "a", ()), ("call", "b", ())), ("call", "c", ())),
        )

    def test_errors(self):
        for src in ["", "f(", "f(1,)", "f(1) &&", "f(1) g(2)", "f 1", "1(2)", "f(1) & g()", "(f()"]:
            with self.subTest(src=src), self.assertRaises(ParseError):
                Parser(src).parse()


class EvalTest(unittest.TestCase):
    def test_truth_tables(self):
        vals = [TRUE, FALSE, UNKNOWN]
        for a in vals:
            for b in vals:
                with self.subTest(a=a, b=b):
                    want_and = FALSE if FALSE in (a, b) else UNKNOWN if UNKNOWN in (a, b) else TRUE
                    want_or = TRUE if TRUE in (a, b) else UNKNOWN if UNKNOWN in (a, b) else FALSE
                    self.assertEqual(ev(f"f({a}) && f({b})")[0], want_and)
                    self.assertEqual(ev(f"f({a}) || f({b})")[0], want_or)
        self.assertEqual(ev("!f(true)")[0], FALSE)
        self.assertEqual(ev("!f(unknown)")[0], UNKNOWN)

    def test_short_circuit(self):
        self.assertEqual(ev("f(false) && g(true)")[1], [("f", ("false",))])
        self.assertEqual(ev("f(true) || g(true)")[1], [("f", ("true",))])

    def test_cache(self):
        e = checks.Evaluator()
        e.cache[("f", ("1",))] = checks.Call("f(1)", TRUE, "out", "")
        calls = []
        self.assertEqual(e.eval(Parser("f(1) && f(1)").parse(), calls), TRUE)
        self.assertEqual([c.stdout for c in calls], ["out", "out"])

    def test_clip(self):
        self.assertEqual(checks.clip("a\n\n  \nb  \n"), "a\nb")
        many = "\n".join(str(i) for i in range(20))
        self.assertEqual(checks.clip(many).splitlines()[-1], "…")
        self.assertEqual(len(checks.clip(many).splitlines()), checks.OUTPUT_LINES + 1)
        self.assertEqual(len(checks.clip("x" * 1000)), checks.OUTPUT_WIDTH)


class MarkerTest(unittest.TestCase):
    def test_marker_and_instructions(self):
        lines = [
            "x = 1;",
            "  ##CHECK: f(1)",
            "  ## Remove the line below.",
            "  ##   and indented detail",
            "  ##",
            "  #  a normal comment ends it",
            "  ## not part of it",
        ]
        m = checks.MARKER.match(lines[1])
        self.assertEqual(m[1].strip(), "f(1)")
        got = []
        for follow in lines[2:]:
            im = checks.INSTRUCTION.match(follow)
            if not im:
                break
            got.append(im[1])
        self.assertEqual(got, ["Remove the line below.", "  and indented detail", ""])
        self.assertIsNone(checks.INSTRUCTION.match("  ##CHECK: g()"))


if __name__ == "__main__":
    unittest.main()
