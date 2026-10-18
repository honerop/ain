"""Tests for the provider-independent end-to-end LLM benchmark."""
from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from tools import llm_bench


SAMPLE = {
    "id": "pair",
    "prompt": "Read two integers and print their sum.",
    "cases": [
        {"stdin": "2 3\n", "stdout": "5\n"},
        {"stdin": "-7 2\n", "stdout": "-5\n"},
    ],
}
WORKING = "import sys\na,b = map(int, sys.stdin.read().split())\nprint(a+b)\n"


class LlmBenchTests(unittest.TestCase):
    def test_prompt_omits_cases(self):
        task = dict(SAMPLE)
        task["cases"] = [{"stdin": "HIDDEN_SECRET_214", "stdout": "secret"},
                         {"stdin": "HIDDEN_SECRET_215", "stdout": "secret"}]
        prompt = llm_bench.prompt_for(task, "ain")
        self.assertIn("Ain", prompt)
        self.assertNotIn("HIDDEN_SECRET_214", prompt)
        self.assertNotIn("HIDDEN_SECRET_215", prompt)

    def test_task_schema_validation(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "tasks.json"
            target.write_text(json.dumps([SAMPLE]), encoding="utf-8")
            self.assertEqual(llm_bench.load_tasks(target)[0]["id"], "pair")
            target.write_text(json.dumps([SAMPLE, SAMPLE]), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "duplicate"):
                llm_bench.load_tasks(target)

    def test_python_candidate_passes_real_process_cases(self):
        with tempfile.TemporaryDirectory() as tmp:
            response = Path(tmp) / "1.json"
            response.write_text(json.dumps({"source": WORKING}), encoding="utf-8")
            row = llm_bench.evaluate(SAMPLE, "python", 1, response, ainc="ainc",
                                     timeout=2, compile_timeout=10)
            self.assertTrue(row["passed"], row)
            self.assertEqual(row["passed_cases"], 2)
            self.assertIsNone(row["usage"])
            self.assertIsNone(row["model"])
            self.assertIn("median_runtime_seconds", row)

    def test_wrong_candidate_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            response = Path(tmp) / "1.json"
            response.write_text(json.dumps({"source": "print(0)\n"}), encoding="utf-8")
            row = llm_bench.evaluate(SAMPLE, "python", 1, response, ainc="ainc",
                                     timeout=2, compile_timeout=10)
            self.assertFalse(row["passed"])
            self.assertEqual(row["status"], "failed")

    def test_missing_response_not_counted_as_submission(self):
        with tempfile.TemporaryDirectory() as tmp:
            row = llm_bench.evaluate(SAMPLE, "python", 1, Path(tmp) / "1.json",
                                     ainc="ainc", timeout=2, compile_timeout=10)
            self.assertEqual(row["status"], "missing")
            summary = llm_bench.summarize([row], [SAMPLE], ["python"])
            self.assertEqual(summary["python"]["submitted_attempts"], 0)
            self.assertIsNone(summary["python"]["input_tokens_known_total"])

    def test_invalid_response_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "1.json"
            path.write_text(json.dumps({"source": "```python\nprint(1)\n```"}),
                            encoding="utf-8")
            row = llm_bench.evaluate(SAMPLE, "python", 1, path, ainc="ainc",
                                     timeout=2, compile_timeout=10)
            self.assertEqual(row["status"], "invalid_response")

    def test_usage_requires_real_supplied_numbers(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "1.json"
            path.write_text(json.dumps({
                "source": WORKING, "usage": {"input_tokens": 77, "output_tokens": 35}
            }), encoding="utf-8")
            row = llm_bench.evaluate(SAMPLE, "python", 1, path, ainc="ainc",
                                     timeout=2, compile_timeout=10)
            summary = llm_bench.summarize([row], [SAMPLE], ["python"])
            self.assertEqual(summary["python"]["input_tokens_known_total"], 77)
            self.assertEqual(summary["python"]["output_tokens_known_total"], 35)

    def test_cli_exports_prompts_without_unsafe_flag(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            tasks = base / "tasks.json"
            tasks.write_text(json.dumps([SAMPLE]), encoding="utf-8")
            self.assertEqual(llm_bench.main([
                "--export-prompts", str(base / "prompts"), "--tasks", str(tasks),
                "--languages", "ain,python",
            ]), 0)
            self.assertTrue((base / "prompts" / "pair" / "ain" / "prompt.txt").is_file())

    def test_execution_requires_explicit_opt_in(self):
        with self.assertRaises(SystemExit) as raised:
            llm_bench.parse_args(["--responses-dir", "somewhere"])
        self.assertEqual(raised.exception.code, 2)

    def test_complete_cli_smoke(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            tasks = base / "tasks.json"
            tasks.write_text(json.dumps([SAMPLE]), encoding="utf-8")
            response_dir = base / "responses" / "pair" / "python"
            response_dir.mkdir(parents=True)
            (response_dir / "1.json").write_text(json.dumps({"source": WORKING}),
                                                 encoding="utf-8")
            report = base / "report.json"
            self.assertEqual(llm_bench.main([
                "--responses-dir", str(base / "responses"),
                "--tasks", str(tasks), "--languages", "python",
                "--report", str(report), "--allow-unsafe-execution",
            ]), 0)
            result = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(result["summary"]["python"]["first_attempt_tasks_passed"], 1)
            self.assertIsNone(result["provider"])
            self.assertIsNone(result["model"])


if __name__ == "__main__":
    unittest.main()
