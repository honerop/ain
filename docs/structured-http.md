# Structured HTTP requests

Babel's native Rust backend supports `httprequest METHOD URL OPTIONS`.

Methods: GET, POST, PUT, PATCH, DELETE, HEAD. The result is a recoverable
JSON-compatible Data object:

```text
{
  "ok": true,
  "value": {
    "status": 201,
    "headers": {"content-type": "application/json"},
    "body": "..."
  },
  "error": ""
}
```

An HTTP status such as 404 still returns `ok: true` because a valid HTTP
response was received. The status code is under `value.status`. Transport
errors, invalid method/URL, unsafe header characters and invalid options return
`ok: false` with a diagnostic in `error`.

Sample Babel:

```text
headers = put record _ "Accept" boxstr "application/json"
opts = put record _ "headers" headers
reply = httprequest "GET" "https://example.com/api" opts
print asbool field reply "ok"
```

Options are a Data record with optional `headers` object (text values),
`body` text (4 MiB limit), `timeout` integer (1..60 seconds), and
`redirects` Boolean (default false). Header names and values are validated.
Redirects, when enabled, are limited to 5 and HTTP(S) protocols. Output is
UTF-8 only with a 4 MiB response cap. Response headers are lowercased; duplicate
headers are concatenated.

The implementation uses the curl executable, without running a shell, and
requires Babel's `network` permission if `BABEL_RESTRICTED=1`.

**Limitations:** operations are synchronous. HTTP/2/HTTP/3 negotiation depends
on the installed curl. This is not a streaming API, does not expose sockets,
does not sandbox network destinations, and is not a replacement for robust
network policies when executing untrusted generated code.

End-to-end tests exercise a real loopback HTTP server, POST with a custom
header/body, status/header parsing, invalid methods and CRLF injection denial.
