# Data serialization in Babel

The native Rust backend supports the following compact operations without
requiring a large dependency tree:

| Babel operation | Type | Behavior |
| --- | --- | --- |
| `csvparse STRING` | Data | Parse comma-separated rows, quoted fields and CRLF/LF |
| `trycsv STRING` | Data result | Parse without panicking; `ok`, `value`, `error` |
| `csvwrite DATA` | String | Write rows of scalar Data values to CSV |
| `utf8bytes STRING` | Data list | Convert UTF-8 text into numeric bytes |
| `frombytes DATA` | Data result | Decode numeric bytes into validated UTF-8 |
| `b64encode DATA` | String | Strict Base64 encoding of a byte list |
| `b64decode STRING` | Data result | Strict Base64 decoding to byte list |
| `zpack DATA` | Data byte list | zlib compress with an 8-byte big-endian size header |
| `zunpack DATA` | Data result | zlib decompress with an 8 MiB output limit |

The native compression operations link against the system zlib library only
when used. CSV operations are implemented in the standard runtime without
extra crates. This is **zlib data compression**, not gzip file packaging.

```text
source = utf8bytes "Hello, Babel"
text = b64encode source
restored = frombytes field b64decode text "value"
print astext field restored "value"
```

Malformed Base64, invalid UTF-8, invalid CSV and decompression errors produce
a Data result with `ok=false`. The strict `csvparse` and `csvwrite`
operations may panic on invalid input or invalid cell types.

**Limits:** TOML, general binary schema definitions, ZIP/TAR and gzip are not
implemented yet. CSV parser input is capped at 8 MiB and 100,000 rows, and
decompression is capped at 8 MiB. Decimal and string formats use UTF-8.

End-to-end generated binary tests verify quoted commas/newlines, Unicode,
UTF-8/Base64 round-trips and malformed-input rejection.
