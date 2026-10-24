# Native TCP and loopback HTTP server

Babel's native backend includes two network operations built on Rust's standard
library, in addition to `httpget`, `httppost`, and `httprequest`.

## TCP client

`tcprequest ADDRESS TEXT` connects to a TCP address such as
`127.0.0.1:8888`, writes UTF-8 text, half-closes the connection, then reads
a bounded UTF-8 reply. It returns a Data result with `ok`, `value` (text),
and `error`.

The payload and reply are each limited to 64 KiB, connections have timeouts,
and at most eight DNS-resolved addresses are attempted. Binary streaming and
persistent socket handles remain future work.

## Local HTTP server

`httpserveonce PORT RESPONSE` binds a **loopback-only** HTTP/1.1 server and
answers one GET/HEAD request, returning a Data result with the method and path.
It accepts ports 1024..65535, waits up to five seconds for a connection, and
limits request headers to 8 KiB and response text to 128 KiB.

```text
print encode httpserveonce 8080 "Hello from Babel"
```

An Babel application can run this on a background native `spawn` task using
a named Data callback; however, this API only serves one request. It does not
yet implement persistent HTTP servers, request bodies, routing, streaming,
TLS termination, or authentication. The fixed-response API is a tested
server foundation, not a production HTTP framework.

All operations require Babel's optional `network` capability when
`BABEL_RESTRICTED=1`. Builtin permission checks are **not** OS network isolation.
Do not expose these APIs to untrusted remote input without an actual sandbox
and appropriate application controls.

Tests use real loopback TCP sockets, a spawned compiled Babel HTTP server,
HTTP request/response assertions, and compile-time argument checks.
