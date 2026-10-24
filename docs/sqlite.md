# Embedded SQLite in Babel

Babel's Rust backend provides two native SQLite operations:

- `sqlexec DATABASE_PATH SQL PARAMETERS`: execute one prepared statement.
- `sqlquery DATABASE_PATH SQL PARAMETERS`: execute one prepared statement and return rows.

Both return a recoverable Data record containing `ok` (boolean), `value`,
and `error` (text). Parameters must be a Data list and are bound through
SQLite's native parameter API, not interpolated into SQL.

```text
database = "people.db"
empty = array _
created = sqlexec database "CREATE TABLE IF NOT EXISTS people(name TEXT)" empty
params = push array _ boxstr "Ada"
inserted = sqlexec database "INSERT INTO people(name) VALUES (?)" params
result = sqlquery database "SELECT name FROM people" empty
print encode field result "value"
```

Queries return a Data list of records, one per row, with column names as keys.
SQL errors, invalid parameter counts and duplicate column names are recoverable.
Each call opens and closes a connection. SQL injection payloads inside bound
text are treated as data.

Limits: single statement per call; at most 10,000 rows returned. BLOB columns,
typed SQL migrations, multiple-statement transactions across calls, connection
pooling, SQLite backup, and SQL schemas are **not yet supported**. These are
native SQLite queries, not a complete database abstraction layer.

The builtin requires the system SQLite3 native library at link time and
database permission when `BABEL_RESTRICTED=1`. This is a builtin permission
check, not filesystem or OS sandbox isolation. Programs that don't call
SQLite don't link the SQLite library.

Tests compile native Babel programs and exercise prepared statements, an SQL
injection-shaped value, persistence, errors, and statement validation.
