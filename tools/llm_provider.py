"""OpenAI-compatible Chat Completions adapter for the Ain LLM benchmark.

Uses only the Python standard library. No API requests occur on import.
Credentials are read from the selected environment variable at call time.
"""
from __future__ import annotations

import ipaddress
import json
import re
import time
from typing import Any
from urllib import error, request
from urllib.parse import urlsplit, urlunsplit

MAX_BODY_BYTES = 2 * 1024 * 1024
RETRY_STATUSES = {429, 500, 502, 503, 504}


class ProviderError(ValueError):
    """API, configuration or response failure; safe to print (no secrets)."""


def endpoint_for(base_url: str, *, allow_insecure_http: bool = False) -> str:
    """Accept /v1, /v1/chat/completions, or a vendor's compatible API prefix."""
    raw = base_url.strip()
    parts = urlsplit(raw)
    if (
        parts.scheme not in {"https", "http"}
        or not parts.netloc
        or not parts.hostname
        or parts.username is not None
        or parts.password is not None
        or parts.query
        or parts.fragment
        or any(c.isspace() for c in raw)
    ):
        raise ProviderError("API base URL must be an absolute HTTP(S) URL without credentials/query/fragment")
    hostname = parts.hostname.lower()
    is_loopback = hostname == "localhost"
    try:
        is_loopback = is_loopback or ipaddress.ip_address(hostname).is_loopback
    except ValueError:
        pass
    if parts.scheme == "http" and not (is_loopback or allow_insecure_http):
        raise ProviderError("HTTP is only allowed for localhost unless --allow-insecure-http is given")
    path = parts.path.rstrip("/")
    if not path.endswith("/chat/completions"):
        path += "/chat/completions"
    return urlunsplit((parts.scheme, parts.netloc, path, "", ""))


def normalize_code(content: str) -> str:
    """Remove one surrounding Markdown code fence, retaining original content elsewhere."""
    match = re.fullmatch(r"\s*\x60{3}[^\n\x60]*\n(.*?)\n?\x60{3}\s*", content, re.DOTALL)
    return match.group(1) if match else content


def parse_completion(response_body: dict[str, Any], requested_model: str) -> dict[str, Any]:
    choices = response_body.get("choices")
    if not isinstance(choices, list) or not choices or not isinstance(choices[0], dict):
        raise ProviderError("API response lacks a Chat Completions choice")
    choice = choices[0]
    message = choice.get("message")
    if not isinstance(message, dict):
        raise ProviderError("API response lacks an assistant message")
    content = message.get("content")
    if isinstance(content, list):
        # Some OpenAI-compatible providers return text parts instead of a string.
        if not all(isinstance(part, dict) and isinstance(part.get("text"), str)
                   for part in content):
            raise ProviderError("API response has unsupported assistant content parts")
        content = "".join(part["text"] for part in content)
    if not isinstance(content, str):
        raise ProviderError("API response has no textual assistant content")
    if not content.strip():
        raise ProviderError("API returned an empty assistant response")

    usage = response_body.get("usage")
    parsed_usage = None
    if isinstance(usage, dict):
        prompt = usage.get("prompt_tokens")
        completion = usage.get("completion_tokens")
        if type(prompt) is int and prompt >= 0 and type(completion) is int and completion >= 0:
            parsed_usage = {"input_tokens": prompt, "output_tokens": completion}
            # These numbers are advisory; token totals are always from actual API usage.
            details = usage.get("completion_tokens_details")
            if isinstance(details, dict):
                reasoning = details.get("reasoning_tokens")
                if type(reasoning) is int and reasoning >= 0:
                    parsed_usage["reasoning_tokens"] = reasoning
    actual_model = response_body.get("model")
    if not isinstance(actual_model, str) or not actual_model:
        actual_model = requested_model
    return {
        "source": normalize_code(content),
        "raw_output": content,
        "provider": "openai-compatible",
        "model": actual_model,
        "usage": parsed_usage,
        "finish_reason": choice.get("finish_reason"),
    }


def generate(
    *, prompt: str, model: str, base_url: str, api_key: str | None,
    timeout: float = 120.0, retries: int = 0, temperature: float | None = None,
    max_output_tokens: int | None = None, max_tokens_field: str = "max_completion_tokens",
    allow_insecure_http: bool = False,
) -> dict[str, Any]:
    endpoint = endpoint_for(base_url, allow_insecure_http=allow_insecure_http)
    if not model.strip() or not model == model.strip():
        raise ProviderError("Model name must be a nonempty string with no surrounding whitespace")
    if timeout <= 0 or retries < 0:
        raise ProviderError("API timeout must be positive and retries nonnegative")
    if temperature is not None and not 0 <= temperature <= 2:
        raise ProviderError("Temperature must be between 0 and 2")
    if max_output_tokens is not None and max_output_tokens < 1:
        raise ProviderError("Output token limit must be positive")
    if max_tokens_field not in {"max_tokens", "max_completion_tokens"}:
        raise ProviderError("Unsupported maximum token parameter")
    body: dict[str, Any] = {
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": False,
    }
    if temperature is not None:
        body["temperature"] = temperature
    if max_output_tokens is not None:
        body[max_tokens_field] = max_output_tokens
    headers = {"Content-Type": "application/json", "Accept": "application/json",
               "User-Agent": "ain-llm-benchmark/1"}
    if api_key is not None:
        headers["Authorization"] = f"Bearer {api_key}"
    encoded = json.dumps(body, ensure_ascii=False).encode("utf-8")
    started = time.perf_counter()
    for attempt in range(retries + 1):
        req = request.Request(endpoint, data=encoded, headers=headers, method="POST")
        try:
            with request.urlopen(req, timeout=timeout) as response:
                payload = response.read(MAX_BODY_BYTES + 1)
            if len(payload) > MAX_BODY_BYTES:
                raise ProviderError("API response exceeds 2 MiB limit")
            try:
                decoded = json.loads(payload)
            except (ValueError, UnicodeDecodeError) as exc:
                raise ProviderError("API returned invalid JSON") from exc
            if not isinstance(decoded, dict):
                raise ProviderError("API returned a non-object JSON response")
            result = parse_completion(decoded, requested_model=model)
            result["generation_seconds"] = round(time.perf_counter() - started, 6)
            return result
        except error.HTTPError as exc:
            # Never print the response body, authorization headers, or URL.
            if exc.code in RETRY_STATUSES and attempt < retries:
                time.sleep(min(2 ** attempt, 8))
                continue
            raise ProviderError(f"Chat Completions request failed: HTTP {exc.code}") from None
        except (error.URLError, TimeoutError, OSError) as exc:
            # Do not retry ambiguous network failures automatically; remote
            # generation might have succeeded and may have incurred charges.
            raise ProviderError(f"Chat Completions connection failed ({type(exc).__name__})") from None
    raise ProviderError("Chat Completions request failed")
