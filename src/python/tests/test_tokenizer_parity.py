"""Validate one shared argv lexing corpus against the package API."""
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from reaterm import run

CASES = json.loads(
    (Path(__file__).resolve().parents[2] / "shared" / "tokenizer-fixtures.json").read_text()
)


class TokenizerParityTests(unittest.TestCase):
    def test_shared_corpus(self):
        for case in CASES:
            with self.subTest(command=case["input"]):
                with patch("reaterm.ReaTerm.run", return_value="ok") as execute:
                    if case.get("error"):
                        with self.assertRaises(ValueError):
                            run(case["input"], binary="unused", decision_model="")
                        execute.assert_not_called()
                    else:
                        self.assertEqual(
                            run(case["input"], binary="unused", decision_model=""), "ok"
                        )
                        self.assertEqual(list(execute.call_args.args), case["argv"])


if __name__ == "__main__":
    unittest.main()
