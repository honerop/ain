#!/usr/bin/env python3
"""Isolated Playwright browser service for Ainex.

JSON Lines over stdin/stdout. It never reuses a personal browser profile.
Operations must be triggered explicitly by the user; there is no arbitrary JS eval.
"""
import ipaddress
import json
import os
import socket
import sys
from pathlib import Path
from urllib.parse import urlsplit

MAX_TEXT = 12000

def validate_url(url: str) -> str:
    if len(url) > 2048:
        raise ValueError("URL is too long")
    parsed = urlsplit(url)
    host = parsed.hostname
    if parsed.scheme not in {"http", "https"} or not host:
        raise ValueError("Only public HTTP(S) URLs are allowed")
    if parsed.username is not None or parsed.password is not None:
        raise ValueError("Embedded URL credentials are not allowed")
    if host.lower() == "localhost" or host.lower().endswith((".localhost", ".local")):
        raise ValueError("Local hostnames are blocked")
    try:
        ip = ipaddress.ip_address(host)
    except ValueError:
        try:
            addresses = [row[4][0] for row in socket.getaddrinfo(host, None)]
        except socket.gaierror as exc:
            raise ValueError(f"Cannot resolve URL hostname: {exc}") from exc
        if not addresses:
            raise ValueError("URL hostname has no addresses")
        if any(not ipaddress.ip_address(a).is_global for a in addresses):
            raise ValueError("Private or local host addresses are blocked")
    else:
        if not ip.is_global:
            raise ValueError("Private or local IP addresses are blocked")
    return url

def safe_route(route):
    try:
        validate_url(route.request.url)
    except (ValueError, TypeError):
        route.abort()
        return
    route.continue_()

def text_snapshot(page) -> str:
    title = page.title()
    url = page.url
    try:
        body = page.locator("body").inner_text(timeout=8000)
    except Exception:
        body = page.locator("html").inner_text(timeout=8000)
    return f"Title: {title}\nURL: {url}\n\n{body[:MAX_TEXT]}"

def link_snapshot(page) -> str:
    links = page.locator("a[href]").evaluate_all(
        """els => els.slice(0, 60).map(a => ({
              text: (a.innerText || a.textContent || '').trim().slice(0, 90),
              href: a.href
           }))"""
    )
    return "\n".join(f"{row['text'] or '(untitled)'}: {row['href']}" for row in links)


# Model-directed browsing uses inspected element handles, never arbitrary JS or
# selectors invented by page text. Handles become invalid if the DOM changes.
ACTIONABLE = "a[href], button, input, textarea, select, [role=button], [role=link], [role=textbox]"
DANGEROUS = ("buy now", "checkout", "check out", "place order", "pay now",
             "confirm order", "complete purchase", "purchase now", "submit order",
             "transfer funds", "delete account", "remove account", "send money",
             "confirm payment", "subscribe now")

def inspect_page(page, elements):
    elements.clear()
    rows = []
    locator = page.locator(ACTIONABLE)
    # Keep bounded output and a stable mapping from displayed IDs to handles.
    for i in range(min(locator.count(), 600)):
        if len(rows) >= 120:
            break
        handle = locator.nth(i).element_handle()
        if handle is None or not handle.is_visible():
            continue
        info = handle.evaluate("""el => {
            const form = el.closest('form');
            return {
                tag: el.tagName.toLowerCase(),
                type: (el.getAttribute('type') || '').toLowerCase(),
                label: (el.getAttribute('aria-label') ||
                    el.getAttribute('placeholder') || el.innerText ||
                    el.getAttribute('value') || el.getAttribute('name') || '').trim(),
                href: el.href || '',
                method: form ? form.method : '',
                action: form ? form.action : ''
            };
        }""")
        index = len(rows) + 1
        elements[index] = handle
        label = " ".join(info["label"].split())[:90]
        href = info["href"][:160]
        rows.append(f"@{index} {info['tag']} {info['type']} {label}" +
                    (f" -> {href}" if href else ""))
    return text_snapshot(page)[:6500] + "\n\nINTERACTIVE ELEMENTS (use @number):\n" + "\n".join(rows)

def element_by_index(elements, argument):
    if not argument.startswith("@") or not argument[1:].isdigit():
        raise ValueError("Use an @number from the latest inspect result")
    handle = elements.get(int(argument[1:]))
    if handle is None or not handle.is_visible():
        raise ValueError("Element is missing or has changed; inspect again")
    return handle

def safe_click(elements, argument):
    element = element_by_index(elements, argument)
    info = element.evaluate("""el => {
        const form = el.closest('form');
        return {
            label: ((el.getAttribute('aria-label') || '') + ' ' +
                (el.innerText || '') + ' ' + (el.getAttribute('value') || '')).toLowerCase(),
            href: el.href || '',
            tag: el.tagName.toLowerCase(),
            type: (el.getAttribute('type') || '').toLowerCase(),
            method: form ? form.method.toLowerCase() : '',
            action: form ? form.action : ''
        };
    }""")
    label = " ".join(info["label"].split())
    destination = (info["href"] + " " + info["action"]).lower()
    if any(word in (label + " " + destination) for word in DANGEROUS):
        raise ValueError("Potential purchase/payment/account action blocked; use a manual browser command")
    if info["href"]:
        validate_url(info["href"])
    # A model may browse GET searches but cannot submit POST forms.
    if (info["tag"] in ("button", "input") and
        info["type"] in ("submit", "image", "") and
        info["method"] not in ("", "get")):
        raise ValueError("Submitting a non-GET form requires a manual browser command")
    element.click(timeout=10000)

def safe_fill(elements, argument):
    index, sep, value = argument.partition("|")
    if not sep or len(value) > 1000:
        raise ValueError("Use @number|text (maximum 1000 characters)")
    element = element_by_index(elements, index.strip())
    info = element.evaluate("""el => ({
        tag: el.tagName.toLowerCase(),
        type: (el.getAttribute('type') || 'text').toLowerCase(),
        name: ((el.getAttribute('name') || '') + ' ' +
               (el.getAttribute('id') || '') + ' ' +
               (el.getAttribute('autocomplete') || '')).toLowerCase()
    })""")
    if info["tag"] not in ("input", "textarea") or info["type"] not in (
        "text", "search", "email", "number", "tel"
    ):
        raise ValueError("Agent can only fill ordinary text/search inputs")
    if any(word in info["name"] for word in (
        "password", "card", "payment", "cc-", "cvv", "cvc", "security", "address",
        "social", "token", "secret", "otp", "verification"
    )):
        raise ValueError("Sensitive form input blocked; fill it manually")
    element.fill(value, timeout=10000)

def serve():
    from playwright.sync_api import sync_playwright
    with sync_playwright() as p:
        browser = None
        context = None
        page = None
        elements = {}
        for line in sys.stdin:
            try:
                request = json.loads(line)
                op = request.get("op", "")
                argument = request.get("argument", "")
                if not isinstance(op, str) or not isinstance(argument, str):
                    raise ValueError("Invalid command")
                if browser is None:
                    browser_exe = os.environ.get("AINEX_CHROMIUM_BROWSER")
                    kwargs = {"headless": os.environ.get("AINEX_BROWSER_HEADLESS", "1") != "0"}
                    if browser_exe:
                        kwargs["executable_path"] = browser_exe
                    browser = p.chromium.launch(**kwargs)
                    context = browser.new_context(accept_downloads=False, service_workers="block")
                    context.route("**/*", safe_route)
                    page = context.new_page()
                    page.set_default_timeout(10000)
                if op == "selftest":
                    page.set_content("""<!doctype html><html><head><title>Ain browser check</title></head>
                        <body><h1 id='state'>Ready</h1><input id='content'>
                        <button id='apply' onclick="document.querySelector('#state').textContent =
                          document.querySelector('#content').value">Apply</button>
                        <a href='https://example.com'>Sample link</a></body></html>""")
                    page.locator("#content").fill("Clicked")
                    page.locator("#apply").click()
                    assert "Clicked" in text_snapshot(page)
                    assert "Sample link" in link_snapshot(page)
                    result = "browser smoke passed"
                elif op == "open":
                    page.goto(validate_url(argument), wait_until="domcontentloaded", timeout=30000)
                    result = text_snapshot(page)
                elif op == "read":
                    result = text_snapshot(page)
                elif op == "inspect":
                    result = inspect_page(page, elements)
                elif op == "click-safe":
                    safe_click(elements, argument)
                    elements.clear()
                    result = text_snapshot(page)
                elif op == "fill-safe":
                    safe_fill(elements, argument)
                    elements.clear()
                    result = text_snapshot(page)
                elif op == "links":
                    result = link_snapshot(page)
                elif op == "back":
                    page.go_back(wait_until="domcontentloaded", timeout=20000)
                    result = text_snapshot(page)
                elif op == "click":
                    if not argument or len(argument) > 500:
                        raise ValueError("A CSS selector is required")
                    page.locator(argument).first.click(timeout=10000)
                    result = text_snapshot(page)
                elif op == "fill":
                    selector, sep, value = argument.partition("|")
                    if not sep or not selector.strip() or len(selector) > 500 or len(value) > 4000:
                        raise ValueError("Use :fill CSS_SELECTOR | text")
                    page.locator(selector.strip()).first.fill(value.lstrip(), timeout=10000)
                    result = text_snapshot(page)
                elif op == "screenshot":
                    name = argument.strip() or "page.png"
                    if Path(name).name != name or not name.endswith(".png") or len(name) > 100:
                        raise ValueError("Screenshot name must be a simple .png filename")
                    location = Path(os.environ.get("XDG_CACHE_HOME", str(Path.home() / ".cache")))
                    target = location / "ainex" / "screenshots" / name
                    target.parent.mkdir(parents=True, exist_ok=True)
                    page.screenshot(path=str(target), full_page=True)
                    result = f"Screenshot saved: {target}"
                else:
                    raise ValueError("Unsupported browser command")
                reply = {"ok": True, "text": result}
            except Exception as exc:
                reply = {"ok": False, "error": str(exc)}
            print(json.dumps(reply, ensure_ascii=False), flush=True)
        if browser is not None:
            browser.close()

def self_test():
    for bad in ["file:///etc/passwd", "http://localhost", "http://127.0.0.1",
                "http://192.168.1.1", "https://user:pass@example.com", "javascript:alert(1)"]:
        try:
            validate_url(bad)
        except ValueError:
            continue
        raise AssertionError(f"Unsafe URL passed validation: {bad}")
    assert validate_url("https://93.184.215.14/path") == "https://93.184.215.14/path"
    print("browser URL safety tests passed")

def browser_smoke():
    """Exercise the real Chromium/Playwright launch without visiting the internet."""
    from playwright.sync_api import sync_playwright
    with sync_playwright() as p:
        options = {"headless": True}
        executable = os.environ.get("AINEX_CHROMIUM_BROWSER")
        if executable:
            options["executable_path"] = executable
        browser = p.chromium.launch(**options)
        try:
            page = browser.new_page()
            page.set_content("""<!doctype html><html><head><title>Ainex browser test</title></head>
                <body><p id="status">Ready</p><input id="message">
                <button id="go" onclick="document.querySelector('#status').textContent =
                document.querySelector('#message').value">Apply</button>
                <a href="https://example.com/">Public link</a></body></html>""")
            assert "Ainex browser test" in text_snapshot(page)
            assert "Public link" in link_snapshot(page)
            page.locator("#message").fill("Browsed")
            page.locator("#go").click()
            assert "Browsed" in text_snapshot(page)
            page.set_content("""<input id="query" aria-label="Product search">
                <button id="search">Search</button>
                <button id="checkout">Place order</button>""")
            handles = {}
            listing = inspect_page(page, handles)
            assert "Product search" in listing
            assert "Place order" in listing
            safe_fill(handles, "@1|book")
            assert page.locator("#query").input_value() == "book"
            handles = {}
            inspect_page(page, handles)
            safe_click(handles, "@2")
            handles = {}
            inspect_page(page, handles)
            try:
                safe_click(handles, "@3")
                raise AssertionError("Purchase should be blocked")
            except ValueError:
                pass
        finally:
            browser.close()
    print("Chromium/Playwright browser smoke test passed")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
    elif "--browser-smoke" in sys.argv:
        browser_smoke()
    else:
        serve()
