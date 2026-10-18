#!/usr/bin/env python3
"""Evaluate externally generated code with real compile/run and held-out cases.

Generate with an OpenAI-compatible API or evaluate supplied JSON responses.
WARNING: compilation and execution of generated code are NOT sandboxed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time
from typing import Any

try:
    from tools import llm_provider
except ModuleNotFoundError:
    # Direct execution: python3 tools/llm_bench.py
    import llm_provider

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TASKS = ROOT / "benchmarks" / "llm" / "tasks.json"
LANGUAGES = {
    "ain": ("main.ain", "Ain"),
    "python": ("main.py", "Python 3"),
    "rust": ("main.rs", "Rust 2021"),
    "typescript": ("main.ts", "TypeScript"),
}
SOURCE_LIMIT = 100_000
OUTPUT_LIMIT = 8_000

AIN_PRIMER = """Ain syntax: one statement per line, prefix calls with fixed arity.
Function: add a b = + a b ; call: add 2 3
Variables: x = 3 ; conditional: if CONDITION THEN ELSE
Read all standard input: input _ (a String); print VALUE prints a line.
String functions: split TEXT DELIMITER, lines TEXT, trim TEXT, len TEXT,
num TEXT (integer conversion), str INTEGER, strjoin LIST DELIMITER.
Typed lists: [1 2 3], listmap, listfilter, listfold, listreverse, sortint,
listlen, listat, listtake, listdrop. Recursion is supported.
No imports, parentheses, semicolons, or explicit type annotations.
Use only operations supported by Ain's native Rust backend.
"""


def prompt_for(task: dict[str, Any], language: str) -> str:
    """Produce the exact reusable prompt; do not include private test cases."""
    language_name = LANGUAGES[language][1]
    notes = {
        "ain": AIN_PRIMER,
        "python": "Use only the Python 3 standard library. Read from stdin.",
        "rust": "Use only the Rust standard library. Write a complete main function.",
        "typescript": (
            "Target Node.js and TypeScript (CommonJS). Do not import packages. "
            "For stdin, declare const require: any; declare const process: any; "
            "and use require('fs').readFileSync(0, 'utf8')."
        ),
    }
    return (
        f"Write a complete single-file program in {language_name}.\n"
        "Output source code only, without markdown or explanations.\n"
        "Read stdin and write ONLY the requested result to stdout.\n"
        "Do not read files other than stdin or use network access.\n\n"
        f"{notes[language]}\n\nTask: {task['prompt']}\n"
    )


def load_tasks(path: Path) -> list[dict[str, Any]]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, list) or not data:
        raise ValueError("tasks.json must contain a nonempty array")
    seen: set[str] = set()
    for task in data:
        if not isinstance(task, dict):
            raise ValueError("task must be an object")
        ident = task.get("id")
        if not isinstance(ident, str) or not ident.replace("-", "").replace("_", "").isalnum():
            raise ValueError(f"invalid task id: {ident!r}")
        if ident in seen:
            raise ValueError(f"duplicate task id: {ident}")
        seen.add(ident)
        if not isinstance(task.get("prompt"), str) or not task["prompt"].strip():
            raise ValueError(f"{ident}: missing prompt")
        cases = task.get("cases")
        if not isinstance(cases, list) or len(cases) < 2:
            raise ValueError(f"{ident}: at least two test cases required")
        for case in cases:
            if not isinstance(case, dict) or not all(
                isinstance(case.get(key), str) for key in ("stdin", "stdout")
            ):
                raise ValueError(f"{ident}: each case needs string stdin and stdout")
    return data


def run_process(
    argv: list[str], *,
    cwd: Path, timeout: float, stdin: str = ""
) -> dict[str, Any]:
    start = time.perf_counter()
    try:
        completed = subprocess.run(
            argv,
            input=stdin,
            text=True,
            encoding="utf-8",
            errors="replace",
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
        return {
            "status": "completed",
            "exit_code": completed.returncode,
            "stdout": completed.stdout[:OUTPUT_LIMIT],
            "stderr": completed.stderr[:OUTPUT_LIMIT],
            "stdout_truncated": len(completed.stdout) > OUTPUT_LIMIT,
            "elapsed_seconds": round(time.perf_counter() - start, 6),
        }
    except subprocess.TimeoutExpired:
        return {
            "status": "timeout", "exit_code": None, "stdout": "", "stderr": "",
            "elapsed_seconds": round(time.perf_counter() - start, 6),
        }
    except OSError as exc:
        return {
            "status": "tool_error", "exit_code": None, "stdout": "",
            "stderr": str(exc)[:OUTPUT_LIMIT],
            "elapsed_seconds": round(time.perf_counter() - start, 6),
        }


def commands(language: str, source: Path, tempdir: Path, ainc: str):
    binary = tempdir / "candidate"
    if language == "ain":
        return [ainc, "build", str(source), "-o", str(binary)], [str(binary)]
    if language == "python":
        return None, [sys.executable, "-I", str(source)]
    if language == "rust":
        return ["rustc", "--edition=2021", "-O", str(source), "-o", str(binary)], [str(binary)]
    if language == "typescript":
        return [
            "tsc", "--target", "ES2020", "--module", "commonjs",
            "--skipLibCheck", "--outDir", str(tempdir), str(source),
        ], ["node", str(tempdir / (source.stem + ".js"))]
    raise ValueError(f"unsupported language: {language}")


def load_response(path: Path) -> dict[str, Any]:
    response = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(response, dict) or not isinstance(response.get("source"), str):
        raise ValueError("response must be a JSON object with a source string")
    source = response["source"]
    if not source.strip() or len(source.encode("utf-8")) > SOURCE_LIMIT:
        raise ValueError("source is empty or exceeds 100 KB")
    if source.lstrip().startswith("```"):
        raise ValueError("source contains markdown fences; submit raw source")
    usage = response.get("usage")
    if usage is not None:
        if not isinstance(usage, dict) or any(
            type(usage.get(k)) is not int or usage[k] < 0
            for k in ("input_tokens", "output_tokens")
            if k in usage
        ):
            raise ValueError("usage token values must be nonnegative integers")
    return response


def generate_responses(tasks: list[dict[str, Any]], args: argparse.Namespace) -> None:
    """Call the model for each task/language/attempt and save raw outputs.

    Existing responses from the same configured model are reused to avoid
    unnecessary API requests and charges. Abort on the first API error.
    """
    key = None if args.no_api_key else os.environ.get(args.api_key_env)
    if key is None and not args.no_api_key:
        raise llm_provider.ProviderError(
            f"Missing API credential: set environment variable {args.api_key_env}"
        )
    for task in tasks:
        for language in args.languages:
            for attempt in range(1, args.attempts + 1):
                target = args.api_responses_dir / task["id"] / language / f"{attempt}.json"
                if target.exists():
                    existing = load_response(target)
                    if (existing.get("requested_model") != args.model or
                            existing.get("provider") != "openai-compatible"):
                        raise llm_provider.ProviderError(
                            f"{target}: response belongs to another model or source; "
                            "choose a separate response directory"
                        )
                    print(f"Reused {task['id']}/{language}/{attempt}")
                    continue
                completion = llm_provider.generate(
                    prompt=prompt_for(task, language),
                    model=args.model, base_url=args.api_base_url,
                    api_key=key, timeout=args.api_timeout,
                    retries=args.api_retries, temperature=args.temperature,
                    max_output_tokens=args.max_output_tokens,
                    max_tokens_field=args.max_tokens_field,
                    allow_insecure_http=args.allow_insecure_http,
                )
                completion["requested_model"] = args.model
                target.parent.mkdir(parents=True, exist_ok=True)
                staged = target.with_suffix(".json.tmp")
                try:
                    staged.write_text(json.dumps(completion, indent=2, ensure_ascii=False) + "\n",
                                      encoding="utf-8")
                    os.replace(staged, target)
                finally:
                    staged.unlink(missing_ok=True)
                print(f"Generated {task['id']}/{language}/{attempt}")


def evaluate(
    task: dict[str, Any], language: str, attempt: int,
    response_path: Path, *, ainc: str, timeout: float, compile_timeout: float
) -> dict[str, Any]:
    row: dict[str, Any] = {
        "task": task["id"], "language": language, "attempt": attempt,
        "status": "missing", "passed": False, "passed_cases": 0,
        "total_cases": len(task["cases"]), "provider": None, "model": None,
        "usage": None, "generation_seconds": None,
    }
    if not response_path.exists():
        return row
    try:
        response = load_response(response_path)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        row.update(status="invalid_response", error=str(exc))
        return row
    row["provider"] = response.get("provider") or None
    row["model"] = response.get("model") or None
    row["usage"] = response.get("usage")  # Unknown is null, never guessed.
    row["generation_seconds"] = response.get("generation_seconds")
    source = response["source"]
    row["source_sha256"] = hashlib.sha256(source.encode("utf-8")).hexdigest()
    row["source_bytes"] = len(source.encode("utf-8"))
    with tempfile.TemporaryDirectory(prefix="ain-llm-bench-") as tmp:
        workdir = Path(tmp)
        source_path = workdir / LANGUAGES[language][0]
        source_path.write_text(source, encoding="utf-8")
        compile_cmd, execute_cmd = commands(language, source_path, workdir, ainc)
        if compile_cmd is not None:
            result = run_process(compile_cmd, cwd=workdir, timeout=compile_timeout)
            row["compile"] = result
            if result["status"] != "completed" or result["exit_code"] != 0:
                row["status"] = "compile_failed"
                return row
        row["status"] = "failed"
        case_results = []
        for case in task["cases"]:
            result = run_process(
                execute_cmd, cwd=workdir, timeout=timeout, stdin=case["stdin"]
            )
            result["passed"] = (
                result["status"] == "completed"
                and result["exit_code"] == 0
                and not result["stdout_truncated"]
                and result["stdout"] == case["stdout"]
            )
            # Test inputs are stored with the task, but not sent to the model.
            case_results.append(result)
        row["cases"] = case_results
        row["passed_cases"] = sum(int(r["passed"]) for r in case_results)
        row["passed"] = row["passed_cases"] == row["total_cases"]
        row["status"] = "passed" if row["passed"] else "failed"
        row["median_runtime_seconds"] = round(
            statistics.median(r["elapsed_seconds"] for r in case_results), 6
        )
    return row


def summarize(rows: list[dict[str, Any]], tasks: list[dict[str, Any]], langs: list[str]) -> dict[str, Any]:
    summary: dict[str, Any] = {}
    for language in langs:
        group = [r for r in rows if r["language"] == language]
        provided = [r for r in group if r["status"] != "missing"]
        passed = [r for r in group if r["passed"]]
        first = [r for r in group if r["attempt"] == 1]
        solved = {
            r["task"] for r in group if r["passed"]
        }
        usage = [
            r["usage"] for r in provided
            if isinstance(r["usage"], dict)
            and all(type(r["usage"].get(k)) is int for k in ("input_tokens", "output_tokens"))
        ]
        summary[language] = {
            "tasks": len(tasks),
            "submitted_attempts": len(provided),
            "passed_attempts": len(passed),
            "first_attempt_tasks_passed": sum(r["passed"] for r in first),
            "solved_within_attempt_budget": len(solved),
            "input_tokens_known_total": sum(u["input_tokens"] for u in usage) if usage else None,
            "output_tokens_known_total": sum(u["output_tokens"] for u in usage) if usage else None,
            "attempts_with_usage": len(usage),
        }
    return summary


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--export-prompts", type=Path, metavar="DIR")
    mode.add_argument("--responses-dir", type=Path, metavar="DIR")
    mode.add_argument("--api-responses-dir", type=Path, metavar="DIR",
                      help="Generate with an OpenAI-compatible API, then evaluate")
    parser.add_argument("--tasks", type=Path, default=DEFAULT_TASKS)
    parser.add_argument("--languages", default="ain,python,rust,typescript")
    parser.add_argument("--attempts", type=int, default=1)
    parser.add_argument("--report", type=Path, default=Path("llm-bench-results.json"))
    parser.add_argument("--ainc", default=os.environ.get("AINC", "ainc"))
    parser.add_argument("--timeout", type=float, default=3)
    parser.add_argument("--compile-timeout", type=float, default=120)
    parser.add_argument("--api-base-url", default="https://api.openai.com/v1",
                        help="Provider base URL, e.g. https://api.openai.com/v1")
    parser.add_argument("--model", help="Model ID for --api-responses-dir")
    parser.add_argument("--api-key-env", default="OPENAI_API_KEY",
                        help="Environment variable containing API key (not key itself)")
    parser.add_argument("--no-api-key", action="store_true",
                        help="Omit Authorization for a local model server")
    parser.add_argument("--allow-insecure-http", action="store_true",
                        help="Allow plaintext HTTP to non-loopback servers (unsafe)")
    parser.add_argument("--api-timeout", type=float, default=120)
    parser.add_argument("--api-retries", type=int, default=0,
                        help="Retry HTTP 429/5xx only; duplicates may be billed")
    parser.add_argument("--temperature", type=float,
                        help="Omitted by default for broad model compatibility")
    parser.add_argument("--max-output-tokens", type=int,
                        help="Optional maximum completion tokens")
    parser.add_argument("--max-tokens-field", choices=["max_completion_tokens", "max_tokens"],
                        default="max_completion_tokens")
    parser.add_argument("--generate-only", action="store_true",
                        help="Generate model source without executing programs")
    parser.add_argument(
        "--allow-unsafe-execution", action="store_true",
        help="Acknowledge that generated code is compiled/run without OS sandboxing",
    )
    args = parser.parse_args(argv)
    args.languages = [x.strip() for x in args.languages.split(",")]
    if not args.languages or len(args.languages) != len(set(args.languages)) or any(
        x not in LANGUAGES for x in args.languages
    ):
        parser.error("--languages must be distinct values from ain,python,rust,typescript")
    if args.attempts < 1 or args.timeout <= 0 or args.compile_timeout <= 0:
        parser.error("attempts and timeouts must be positive")
    if args.api_responses_dir:
        if not args.model or not args.model.strip():
            parser.error("--model is required with --api-responses-dir")
        if not args.api_key_env.isidentifier():
            parser.error("--api-key-env must be a valid environment variable name")
        if args.api_timeout <= 0 or not 0 <= args.api_retries <= 5:
            parser.error("API timeout must be positive and retries between 0 and 5")
        if args.temperature is not None and not 0 <= args.temperature <= 2:
            parser.error("--temperature must be between 0 and 2")
        if args.max_output_tokens is not None and args.max_output_tokens <= 0:
            parser.error("--max-output-tokens must be positive")
        try:
            llm_provider.endpoint_for(args.api_base_url,
                                      allow_insecure_http=args.allow_insecure_http)
        except llm_provider.ProviderError as exc:
            parser.error(str(exc))
    elif args.generate_only:
        parser.error("--generate-only requires --api-responses-dir")
    if (args.responses_dir or (args.api_responses_dir and not args.generate_only)) and not args.allow_unsafe_execution:
        parser.error("generated programs are NOT sandboxed; supply --allow-unsafe-execution only in an isolated environment")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    tasks = load_tasks(args.tasks)
    if args.export_prompts:
        for task in tasks:
            for language in args.languages:
                location = args.export_prompts / task["id"] / language
                location.mkdir(parents=True, exist_ok=True)
                (location / "prompt.txt").write_text(prompt_for(task, language), encoding="utf-8")
        print(f"Wrote {len(tasks) * len(args.languages)} prompts into {args.export_prompts}")
        return 0

    if args.api_responses_dir:
        generate_responses(tasks, args)
        if args.generate_only:
            print(f"Saved responses in {args.api_responses_dir} (no generated code executed)")
            return 0
        args.responses_dir = args.api_responses_dir

    rows = [
        evaluate(
            task, language, attempt,
            args.responses_dir / task["id"] / language / f"{attempt}.json",
            ainc=args.ainc, timeout=args.timeout,
            compile_timeout=args.compile_timeout,
        )
        for task in tasks for language in args.languages
        for attempt in range(1, args.attempts + 1)
    ]
    report = {
        "schema": 1,
        "created_utc": dt_utc(),
        "provider": "openai-compatible" if args.api_responses_dir else None,
        "model": args.model if args.api_responses_dir else None,
        "notes": (
            "Usage is copied from API responses, never inferred. "
            "Generated candidate execution is not sandboxed."
        ),
        "config": {
            "task_ids": [task["id"] for task in tasks],
            "generation_mode": "api" if args.api_responses_dir else "manual",
            "languages": args.languages, "attempts": args.attempts,
            "runtime_timeout_seconds": args.timeout,
            "compile_timeout_seconds": args.compile_timeout,
        },
        "summary": summarize(rows, tasks, args.languages),
        "results": rows,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"Report: {args.report}")
    for language, data in report["summary"].items():
        print(
            f"{language:10} first attempt: {data['first_attempt_tasks_passed']}/{data['tasks']}"
            f"  solved within {args.attempts} attempts: {data['solved_within_attempt_budget']}/{data['tasks']}"
            f"  submitted attempts: {data['submitted_attempts']}"
        )
    return 0


def dt_utc() -> str:
    from datetime import datetime, timezone
    return datetime.now(timezone.utc).isoformat()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"llm-bench: {exc}", file=sys.stderr)
        raise SystemExit(2)
