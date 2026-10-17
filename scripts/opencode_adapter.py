#!/usr/bin/env python3
"""OpenCode JSONL adapter for scripts/agent_loop.py (not an execution sandbox)."""
import json
import os
import re
import subprocess
import sys

MODEL = os.environ.get("AIN_OPENCODE_MODEL", "opencode/mimo-v2.6-flash-free")
BIN = os.environ.get("AIN_OPENCODE_BIN", "opencode")
MAX_PROMPT = 128 * 1024
MAX_REPLY = 512 * 1024

LANGUAGE_NOTES = """Ain is a compact, typed, fixed-arity prefix language.
Write only raw Ain source, with no markdown, explanations, or commentary.
One definition or expression per line. Functions: add a b = + a b
Call: print add 2 3
Conditionals: if > n 0 n 0
Strings: "hello". Unit value: _. No fn keyword or parentheses.
Primitive operations: + - * / % == != < <= > >= and or not
Print: print EXPRESSION. Assert: assert BOOLEAN.
Prefer supported builtins. Do not invent syntax or imports.
"""

def make_prompt(request):
    prompt = str(request.get("prompt", ""))
    previous = str(request.get("previous_source", ""))
    diagnostics = request.get("diagnostics", [])
    if not prompt.strip():
        raise ValueError("missing prompt")
    chunks = [LANGUAGE_NOTES, "Task:\n" + prompt]
    if previous:
        chunks.append("Previous Ain source:\n" + previous)
        chunks.append("Compiler diagnostics (fix these):\n" + json.dumps(diagnostics, ensure_ascii=False))
    combined = "\n\n".join(chunks)
    if len(combined.encode("utf-8")) > MAX_PROMPT:
        raise ValueError("OpenCode request exceeds 128 KiB")
    return combined

def extract_source(output):
    parts = []
    for line in output.splitlines():
        if not line.lstrip().startswith("{"):
            continue
        event = json.loads(line)
        if event.get("type") == "error":
            raise ValueError("OpenCode reported an error: " + str(event)[:350])
        if event.get("type") == "text":
            text = event.get("part", {}).get("text")
            if isinstance(text, str):
                parts.append(text)
    source = "".join(parts).strip()
    if not source:
        raise ValueError("OpenCode returned no text events")
    match = re.fullmatch(r"~~~(?:ain|text)?\s*\n([\s\S]*?)\n~~~|```(?:ain|text)?\s*\n([\s\S]*?)\n```", source)
    if match:
        source = match.group(1) or match.group(2)
    if len(source.encode("utf-8")) > MAX_REPLY:
        raise ValueError("OpenCode response exceeds 512 KiB")
    return source.rstrip() + "\n"

def main():
    try:
        request = json.load(sys.stdin)
        if request.get("protocol") != "ain-generate-v1":
            raise ValueError("unexpected generator protocol")
        prompt = make_prompt(request)
        command = [BIN, "run", "--model", MODEL, "--format", "json",
                   "--agent", "plan", "--", prompt]
        proc = subprocess.run(command, capture_output=True, text=True, timeout=180)
        if proc.returncode != 0:
            raise ValueError("OpenCode failed: " + proc.stderr[-1000:])
        sys.stdout.write(extract_source(proc.stdout))
        return 0
    except (OSError, ValueError, subprocess.TimeoutExpired, json.JSONDecodeError) as error:
        print("OpenCode adapter: " + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    sys.exit(main())
