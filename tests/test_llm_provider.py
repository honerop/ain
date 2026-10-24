"""Offline OpenAI-compatible benchmark tests: no credentials or network required."""
from __future__ import annotations

import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from urllib.error import HTTPError

from tools import llm_bench, llm_provider

TASK = {
    "id": "sum-two", "prompt": "Print the sum of two integers from stdin.",
    "cases": [
        {"stdin": "2 3\n", "stdout": "5\n"},
        {"stdin": "-3 1\n", "stdout": "-2\n"}
    ],
}
CODE = "import sys\na,b=map(int,sys.stdin.read().split())\nprint(a+b)\n"


class FakeResponse:
    def __init__(self, data):
        self.content = json.dumps(data).encode()

    def __enter__(self):
        return self

    def __exit__(self, *args):
        return False

    def read(self, n):
        return self.content[:n]


class ProviderTests(unittest.TestCase):
    def test_endpoints(self):
        self.assertEqual(
            llm_provider.endpoint_for("https://example.com/custom/v1/"),
            "https://example.com/custom/v1/chat/completions"
        )
        self.assertEqual(
            llm_provider.endpoint_for("https://api.openai.com/v1/chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        )
        self.assertEqual(
            llm_provider.endpoint_for("http://localhost:8011/v1"),
            "http://localhost:8011/v1/chat/completions"
        )
        for url in ("http://example.com/v1",
                    "https://key:password@example.com/v1",
                    "https://example.com/v1?api_key=secret"):
            with self.subTest(url=url), self.assertRaises(llm_provider.ProviderError):
                llm_provider.endpoint_for(url)

    def test_parse_markdown_and_provider_usage(self):
        fence = chr(96) * 3
        result = llm_provider.parse_completion({
            "model": "test-model",
            "choices": [{"message": {"content": fence + "python\nprint(1)\n" + fence},
                         "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 35, "completion_tokens": 15,
                      "completion_tokens_details": {"reasoning_tokens": 7}},
        }, "requested-model")
        self.assertEqual(result["source"], "print(1)")
        self.assertEqual(result["model"], "test-model")
        self.assertEqual(result["usage"], {
            "input_tokens": 35, "output_tokens": 15, "reasoning_tokens": 7
        })
        self.assertEqual(result["raw_output"], fence + "python\nprint(1)\n" + fence)

    def test_content_parts_missing_usage_and_invalid_responses(self):
        r = llm_provider.parse_completion({
            "choices": [{"message": {"content": [
                {"type": "text", "text": "print"}, {"type": "text", "text": "(1)"}
            ]}}]
        }, "demo")
        self.assertEqual(r["source"], "print(1)")
        self.assertIsNone(r["usage"])
        for body in ({}, {"choices": []},
                     {"choices": [{"message": {"content": None}}]},
                     {"choices": [{"message": {"content": ""}}]}):
            with self.subTest(body=body), self.assertRaises(llm_provider.ProviderError):
                llm_provider.parse_completion(body, "demo")

    def test_request_payload_and_secret_redaction(self):
        body = {
            "model": "fake", "choices": [{"message": {"content": CODE},
                                          "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 100, "completion_tokens": 25},
        }
        def urlopen(req, timeout):
            self.assertEqual(req.full_url, "https://example.org/v1/chat/completions")
            self.assertEqual(req.get_method(), "POST")
            self.assertEqual(req.get_header("Authorization"), "Bearer fake-secret")
            payload = json.loads(req.data)
            self.assertEqual(payload["model"], "fake")
            self.assertEqual(payload["messages"][0]["content"], "Build program")
            self.assertEqual(payload["temperature"], 0.2)
            self.assertEqual(payload["max_completion_tokens"], 400)
            return FakeResponse(body)
        with mock.patch.object(llm_provider.request, "urlopen", side_effect=urlopen):
            result = llm_provider.generate(
                prompt="Build program", model="fake", api_key="fake-secret",
                base_url="https://example.org/v1", temperature=0.2,
                max_output_tokens=400,
            )
        self.assertEqual(result["usage"], {"input_tokens": 100, "output_tokens": 25})
        self.assertNotIn("fake-secret", json.dumps(result))

    def test_errors_do_not_expose_provider_body(self):
        exc = HTTPError("https://example.com/v1", 401,
                        "secret request", {}, io.BytesIO(b"my-api-key"))
        with mock.patch.object(llm_provider.request, "urlopen", side_effect=exc):
            with self.assertRaises(llm_provider.ProviderError) as caught:
                llm_provider.generate(prompt="x", model="m", api_key="my-api-key",
                                      base_url="https://example.com/v1")
        self.assertEqual(str(caught.exception), "Chat Completions request failed: HTTP 401")

    def test_retries_429_only_when_enabled(self):
        ok = {"choices": [{"message": {"content": CODE}}]}
        exc = HTTPError("https://example.com/v1", 429, "too many", {}, io.BytesIO())
        with mock.patch.object(llm_provider.request, "urlopen",
                               side_effect=[exc, FakeResponse(ok)]) as mocked, \
             mock.patch.object(llm_provider.time, "sleep"):
            result = llm_provider.generate(
                prompt="x", model="m", api_key=None,
                base_url="https://example.com/v1", retries=1
            )
        self.assertEqual(mocked.call_count, 2)
        self.assertEqual(result["source"], CODE)

    def test_cli_generate_only_resume_and_evaluate(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            tasks = root / "tasks.json"
            tasks.write_text(json.dumps([TASK]))
            output = root / "responses"
            opts = ["--tasks", str(tasks), "--languages", "python",
                    "--api-responses-dir", str(output),
                    "--model", "example-model", "--api-key-env", "MY_TEST_API_KEY",
                    "--attempts", "2"]
            completion = {
                "source": CODE, "provider": "openai-compatible",
                "model": "example-model",
                "usage": {"input_tokens": 17, "output_tokens": 20},
                "generation_seconds": 0.001,
            }
            with mock.patch.dict(os.environ, {"MY_TEST_API_KEY": "not-in-report"}), \
                 mock.patch.object(llm_provider, "generate", return_value=completion) as gen:
                self.assertEqual(llm_bench.main(opts + ["--generate-only"]), 0)
                self.assertEqual(llm_bench.main(opts + ["--generate-only"]), 0)
                self.assertEqual(gen.call_count, 2)  # attempts saved and reused
                report = root / "report.json"
                self.assertEqual(llm_bench.main(
                    opts + ["--report", str(report), "--allow-unsafe-execution"]
                ), 0)
                self.assertEqual(gen.call_count, 2)
            data = json.loads(report.read_text())
            self.assertEqual(data["provider"], "openai-compatible")
            self.assertEqual(data["model"], "example-model")
            self.assertEqual(data["summary"]["python"]["first_attempt_tasks_passed"], 1)
            self.assertEqual(data["summary"]["python"]["input_tokens_known_total"], 34)
            self.assertNotIn("not-in-report", json.dumps(data))

    def test_generated_programs_cannot_read_api_keys_from_environment(self):
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.dict(os.environ, {
                "OPENAI_API_KEY": "should-never-reach-the-child",
                "MY_TEST_API_KEY": "also-not-visible",
            }):
                result = llm_bench.run_process([
                    os.sys.executable, "-c",
                    "import os;print(os.getenv('OPENAI_API_KEY'));print(os.getenv('MY_TEST_API_KEY'))",
                ], cwd=Path(tmp), timeout=5)
            self.assertEqual(result["status"], "completed")
            self.assertEqual(result["stdout"], "None\nNone\n")


    def test_requires_model_and_key_and_rejects_mixed_responses(self):
        with self.assertRaises(SystemExit):
            llm_bench.parse_args(["--api-responses-dir", "out", "--generate-only"])
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            taskfile = home / "tasks.json"
            taskfile.write_text(json.dumps([TASK]))
            args = ["--tasks", str(taskfile), "--languages", "babel",
                    "--api-responses-dir", str(home / "out"),
                    "--model", "demo", "--api-key-env", "EMPTY_KEY_TEST",
                    "--generate-only"]
            with mock.patch.dict(os.environ, {}, clear=True):
                with self.assertRaisesRegex(llm_provider.ProviderError, "Missing API credential"):
                    llm_bench.main(args)
            target = home / "out" / "sum-two" / "babel"
            target.mkdir(parents=True)
            (target / "1.json").write_text(json.dumps({
                "source": "print 1", "provider": "openai-compatible",
                "requested_model": "different",
            }))
            with mock.patch.dict(os.environ, {"EMPTY_KEY_TEST": "s"}):
                with self.assertRaisesRegex(llm_provider.ProviderError, "another model"):
                    llm_bench.main(args)


if __name__ == "__main__":
    unittest.main()
