#!/usr/bin/env python3
"""Compare Ain Rust/LLVM backends. This executes programs; sandbox untrusted code."""
import argparse
import json
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CASES = ["examples/arithmetic.ain", "examples/branches.ain", "examples/recursive.ain"]

def source_stats(text):
    return {"utf8_bytes": len(text.encode("utf-8")),
            "whitespace_units_not_tokens": len(text.split())}

def model_metrics(path):
    if path is None:
        return None
    data = [json.loads(s) for s in path.read_text(encoding="utf-8").splitlines() if s.strip()]
    def total(key):
        if any(not isinstance(x.get(key), int) or isinstance(x[key], bool) or x[key] < 0 for x in data):
            return None
        return sum(x[key] for x in data) if data else None
    return {"attempts": len(data),
            "successful_attempts": sum(x.get("ok") is True for x in data),
            "prompt_tokens_reported": total("prompt_tokens"),
            "completion_tokens_reported": total("completion_tokens")}

def timed(command, timeout):
    start = time.perf_counter()
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=timeout)
    return result, round((time.perf_counter() - start) * 1000, 3)

def case_result(source_file, compiler, backends, runs, timeout):
    result = {"case": str(source_file.relative_to(ROOT)) if source_file.is_relative_to(ROOT) else str(source_file),
              "source": source_stats(source_file.read_text(encoding="utf-8")), "backends": {}, "equivalent": None}
    with tempfile.TemporaryDirectory(prefix="ain-benchmark-") as tmp:
        for backend in backends:
            program = Path(tmp) / ("app-" + backend + (".exe" if sys.platform == "win32" else ""))
            command = [compiler, "build" if backend == "rust" else "build-llvm",
                       str(source_file), "-o", str(program)]
            build, ms = timed(command, timeout)
            entry = {"compile_ms": ms, "compile_ok": build.returncode == 0}
            if build.returncode != 0:
                entry["error"] = build.stderr[-2000:]
            else:
                outputs, codes, timings = [], [], []
                for _ in range(runs):
                    run, elapsed = timed([str(program)], timeout)
                    outputs.append(run.stdout)
                    codes.append(run.returncode)
                    timings.append(elapsed)
                entry.update({"stdout": outputs[0], "exit_codes": codes,
                              "stable_stdout": len(set(outputs)) == 1,
                              "median_run_ms": statistics.median(timings)})
            result["backends"][backend] = entry
    if len(backends) == 2 and all(x["compile_ok"] for x in result["backends"].values()):
        a, b = [result["backends"][name] for name in backends]
        result["equivalent"] = (a["stdout"] == b["stdout"] and a["exit_codes"] == b["exit_codes"]
                                and a["stable_stdout"] and b["stable_stdout"])
    return result

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default=str(ROOT / "target" / "debug" / ("ainc.exe" if sys.platform == "win32" else "ainc")))
    parser.add_argument("--backend", choices=["rust", "llvm", "both"], default="both")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--strict", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--model-trace", type=Path, help="JSONL records with real provider usage")
    parser.add_argument("cases", nargs="*", help="Ain source paths")
    args = parser.parse_args(argv)
    if args.runs < 1 or args.timeout <= 0:
        parser.error("runs and timeout must be positive")
    compiler = str(Path(args.compiler).resolve())
    if not Path(compiler).is_file():
        parser.error("compiler missing; run cargo build --bin ainc")
    backends = ["rust", "llvm"] if args.backend == "both" else [args.backend]
    report = {"schema": 1, "note": "wall times depend on the host; source units are not LLM tokens",
              "model_usage": model_metrics(args.model_trace), "cases": []}
    failed = False
    for case in args.cases or CASES:
        file = (ROOT / case).resolve()
        if not file.is_file():
            parser.error("missing case: " + case)
        try:
            outcome = case_result(file, compiler, backends, args.runs, args.timeout)
        except (subprocess.TimeoutExpired, OSError) as error:
            outcome = {"case": str(file), "error": str(error)}
        report["cases"].append(outcome)
        entries = outcome.get("backends", {})
        if outcome.get("error") or outcome.get("equivalent") is False or any(
                not item["compile_ok"] or not item.get("stable_stdout", False)
                or any(code != 0 for code in item.get("exit_codes", []))
                for item in entries.values()):
            failed = True
    output = json.dumps(report, indent=2, ensure_ascii=False) + "\n"
    print(output, end="")
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output, encoding="utf-8")
    return int(failed and args.strict)

if __name__ == "__main__":
    sys.exit(main())
