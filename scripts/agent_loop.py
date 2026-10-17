#!/usr/bin/env python3
"""Bounded generator -> ainc check-json -> repair loop. Never runs generated Ain."""
import argparse
import json
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path

def check(compiler, source, timeout):
    with tempfile.TemporaryDirectory(prefix="ain-agent-") as root:
        file = Path(root) / "candidate.ain"
        file.write_text(source, encoding="utf-8")
        try:
            p = subprocess.run(compiler + ["check-json", str(file)],
                               capture_output=True, text=True, timeout=timeout)
            data = json.loads(p.stdout)
            if isinstance(data, dict) and isinstance(data.get("ok"), bool):
                if p.returncode != 0 and data["ok"]:
                    raise ValueError("compiler exit code disagrees with response")
                return data
            raise ValueError("unexpected compiler response")
        except (OSError, subprocess.TimeoutExpired, ValueError) as error:
            return {"ok": False, "diagnostics": [{"code": "E_TOOL", "message": str(error)}]}

def repair(prompt, target, generator, compiler, attempts=3, timeout=60, max_bytes=1048576):
    source, diagnostics = "", []
    for attempt in range(1, attempts + 1):
        request = {"protocol": "ain-generate-v1", "prompt": prompt, "attempt": attempt,
                   "previous_source": source, "diagnostics": diagnostics}
        try:
            output = subprocess.run(generator, input=json.dumps(request),
                                    text=True, capture_output=True, timeout=timeout)
            if output.returncode:
                diagnostics = [{"code": "E_GENERATOR", "message": output.stderr[-1600:]}]
            elif len(output.stdout.encode("utf-8")) > max_bytes:
                diagnostics = [{"code": "E_SOURCE_SIZE", "message": "generated source is too large"}]
            elif not output.stdout.strip():
                diagnostics = [{"code": "E_EMPTY_SOURCE", "message": "generator returned empty source"}]
            else:
                source = output.stdout
                verified = check(compiler, source, timeout)
                diagnostics = verified.get("diagnostics", [])
                if verified["ok"]:
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text(source, encoding="utf-8")
                    return {"ok": True, "attempts": attempt, "output": str(target)}
        except (OSError, subprocess.TimeoutExpired) as error:
            diagnostics = [{"code": "E_GENERATOR", "message": str(error)}]
        print(json.dumps({"attempt": attempt, "diagnostics": diagnostics}), file=sys.stderr)
    return {"ok": False, "attempts": attempts, "diagnostics": diagnostics}

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--prompt-file", required=True, type=Path)
    p.add_argument("--output", required=True, type=Path)
    p.add_argument("--ainc", default="ainc", help="Compiler argv, split without a shell")
    p.add_argument("--attempts", type=int, default=3)
    p.add_argument("--timeout", type=float, default=60)
    p.add_argument("--max-source-bytes", type=int, default=1048576)
    p.add_argument("generator", nargs=argparse.REMAINDER, help="After --, generator command accepting JSON stdin")
    args = p.parse_args()
    if args.attempts < 1 or args.timeout <= 0 or args.max_source_bytes < 1:
        p.error("attempts, timeout and source limit must be positive")
    generator = args.generator[1:] if args.generator and args.generator[0] == "--" else args.generator
    if not generator:
        p.error("supply a generator command after --")
    result = repair(args.prompt_file.read_text(encoding="utf-8"), args.output, generator,
                    shlex.split(args.ainc), args.attempts, args.timeout, args.max_source_bytes)
    print(json.dumps(result))
    return 0 if result["ok"] else 1

if __name__ == "__main__":
    sys.exit(main())
