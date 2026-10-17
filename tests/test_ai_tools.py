import json
import sys
import tempfile
import unittest
from pathlib import Path
from scripts.agent_loop import repair
from scripts.benchmark import model_metrics, source_stats

class AIToolingTests(unittest.TestCase):
    def test_source_units_are_explicitly_not_tokens(self):
        self.assertEqual(source_stats("print 123\n"), {"utf8_bytes": 10, "whitespace_units_not_tokens": 2})

    def test_model_usage_comes_only_from_provider_trace(self):
        with tempfile.TemporaryDirectory() as tmp:
            trace = Path(tmp) / "usage.jsonl"
            trace.write_text('{"ok":true,"prompt_tokens":20,"completion_tokens":10}\n'
                             '{"ok":false,"prompt_tokens":8,"completion_tokens":3}\n')
            data = model_metrics(trace)
            self.assertEqual(data["attempts"], 2)
            self.assertEqual(data["successful_attempts"], 1)
            self.assertEqual(data["prompt_tokens_reported"], 28)
            self.assertEqual(data["completion_tokens_reported"], 13)

    def test_repairs_using_diagnostics_without_running_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            model, compiler, result = root / "generator.py", root / "check.py", root / "result.ain"
            model.write_text("import json,sys\nr=json.load(sys.stdin)\n"
                             "print('print 42' if r['diagnostics'] else 'print unknown')\n")
            compiler.write_text("import json,sys\ns=open(sys.argv[-1]).read()\n"
                                "ok='unknown' not in s\n"
                                "print(json.dumps({'ok':ok,'diagnostics':[] if ok else "
                                "[{'code':'E_UNKNOWN_VARIABLE','message':'unknown variable'}]}))\n"
                                "sys.exit(0 if ok else 1)\n")
            outcome = repair("Print 42", result, [sys.executable, str(model)],
                             [sys.executable, str(compiler)], attempts=3)
            self.assertTrue(outcome["ok"])
            self.assertEqual(outcome["attempts"], 2)
            self.assertEqual(result.read_text(), "print 42\n")

if __name__ == "__main__":
    unittest.main()
