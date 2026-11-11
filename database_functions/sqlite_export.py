"""Export a PostgreSQL or MariaDB/MySQL Pinepods database as a SQLite server backup.

Moving a server to SQLite: run this against the old database (the same DB_* variables the
server uses), start a new SQLite server on the same Pinepods version, and restore the file
there from Settings > Restore Server.

    pinepods-db-setup --export-sqlite /opt/pinepods/backups/sqlite_export.sql     (in the container)
    python -m database_functions.sqlite_export sqlite_export.sql                   (from the repo)

The file has the format the SQLite server writes for its own backups: the SQLite schema (from
the baseline), then every row, with values in the form the SQLite server stores them. Login
sessions aren't exported.
"""

import json
import math
import os
import sqlite3
import sys
from datetime import date, datetime, time, timedelta, timezone
from decimal import Decimal

from database_functions.sqlite_baseline import BASELINE_PATH, split_statements

HEADER = "-- Pinepods SQLite backup"
SKIP_DATA = {"sessions", "gpoddersessions"}
MIGRATIONS_TABLE = (
    'CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT, '
    "checksum TEXT NOT NULL, applied_at TIMESTAMP, execution_time_ms INTEGER)"
)


def target_schema():
    """The SQLite schema: (table statements, index statements, {table: [(column, type)]})."""
    with open(BASELINE_PATH) as f:
        script = f.read()
    tables, indexes = [], []
    for stmt in split_statements(script):
        head = stmt.lstrip().upper()
        if head.startswith("CREATE TABLE"):
            tables.append(stmt.strip().rstrip(";"))
        elif head.startswith("CREATE") and "INDEX" in head.split("(")[0]:
            indexes.append(stmt.strip().rstrip(";"))
    db = sqlite3.connect(":memory:")
    for stmt in tables:
        db.execute(stmt)
    columns = {}
    for (name,) in db.execute("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'"):
        columns[name] = [(r[1], (r[2] or "").upper()) for r in db.execute(f'PRAGMA table_info("{name}")')]
    db.close()
    return tables, indexes, columns


def timestamp_text(v: datetime) -> str:
    if v.tzinfo is not None:
        v = v.astimezone(timezone.utc).replace(tzinfo=None)
    text = v.strftime("%Y-%m-%d %H:%M:%S")
    return text + (".%06d" % v.microsecond if v.microsecond else "")


def quote(s: str) -> str:
    return "'" + s.replace("'", "''") + "'"


def literal(v, decl: str) -> str:
    """`v` as a SQLite literal for a column declared `decl`."""
    if v is None:
        return "NULL"
    if isinstance(v, memoryview):
        v = v.tobytes()
    if decl == "BOOLEAN":
        if isinstance(v, (bytes, bytearray)):
            v = int.from_bytes(v, "little")
        if isinstance(v, str):
            v = v.strip().lower() in ("1", "t", "true", "y", "yes")
        return "1" if v else "0"
    if isinstance(v, bool):
        return "1" if v else "0"
    if isinstance(v, (bytes, bytearray)):
        if decl == "BLOB":
            return "X'" + bytes(v).hex() + "'"
        v = bytes(v).decode("utf-8", "replace")
    if isinstance(v, datetime):
        return quote(timestamp_text(v))
    if isinstance(v, (date, time)):
        return quote(v.isoformat())
    if isinstance(v, timedelta):
        return repr(v.total_seconds())
    if isinstance(v, Decimal):
        v = float(v)
    if isinstance(v, float):
        if math.isnan(v):
            return "NULL"
        if math.isinf(v):
            return "9e999" if v > 0 else "-9e999"
        return repr(v)
    if isinstance(v, int):
        return str(v)
    if isinstance(v, (list, tuple, dict)):
        return quote(json.dumps(v, default=str))
    return quote(str(v))


def source_tables(cursor, db_type: str) -> dict:
    """{lowercase name: actual name} of the source database's tables."""
    if db_type == "postgresql":
        cursor.execute("SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' AND table_type = 'BASE TABLE'")
    else:
        cursor.execute("SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE'")
    out = {}
    for (name,) in cursor.fetchall():
        name = name.decode() if isinstance(name, (bytes, bytearray)) else name
        out[name.lower()] = name
    return out


def export(conn, db_type: str, out_path: str) -> dict:
    """Write the export of `conn` to `out_path`; returns rows written per table."""
    tables, indexes, columns = target_schema()
    quote_ident = (lambda n: f'"{n}"') if db_type == "postgresql" else (lambda n: f"`{n}`")
    cursor = conn.cursor()
    present = source_tables(cursor, db_type)
    counts = {}
    tmp_path = out_path + ".partial"
    with open(tmp_path, "w", encoding="utf-8", newline="\n") as out:
        out.write(HEADER + "\n")
        for stmt in tables:
            out.write(stmt + ";\n")
        out.write(MIGRATIONS_TABLE + ";\n")

        targets = sorted(columns.items()) + [("schema_migrations", [
            ("version", "TEXT"), ("name", "TEXT"), ("description", "TEXT"), ("checksum", "TEXT"),
            ("applied_at", "TIMESTAMP"), ("execution_time_ms", "INTEGER"),
        ])]
        for table, cols in targets:
            source = present.get(table.lower())
            if source is None or table.lower() in SKIP_DATA:
                continue
            # Rows stream from a server-side cursor where the driver has one.
            rows = conn.cursor(name=f"export_{table.lower()}") if db_type == "postgresql" else conn.cursor()
            rows.execute(f"SELECT * FROM {quote_ident(source)}")
            source_cols = [d[0].decode() if isinstance(d[0], (bytes, bytearray)) else d[0] for d in rows.description]
            position = {c.lower(): i for i, c in enumerate(source_cols)}
            shared = [(c, t) for c, t in cols if c.lower() in position]
            names = ", ".join(f'"{c}"' for c, _ in shared)
            prefix = f'INSERT INTO "{table}" ({names}) VALUES('
            n = 0
            while True:
                batch = rows.fetchmany(1000)
                if not batch:
                    break
                for row in batch:
                    values = ",".join(literal(row[position[c.lower()]], t) for c, t in shared)
                    out.write(prefix + values + ");\n")
                    n += 1
            rows.close()
            counts[table] = n
        for stmt in indexes:
            out.write(stmt + ";\n")
    os.replace(tmp_path, out_path)
    cursor.close()
    return counts


def main(argv) -> int:
    if len(argv) != 1:
        print("usage: python -m database_functions.sqlite_export <output file>", file=sys.stderr)
        return 2
    from database_functions.migrations import get_migration_manager

    manager = get_migration_manager()
    if manager.db_type == "sqlite":
        print("This server already uses SQLite; its own backups restore directly.", file=sys.stderr)
        return 1
    db_type = "postgresql" if manager.db_type in ("postgresql", "postgres") else "mysql"
    counts = export(manager.get_connection(), db_type, argv[0])
    total = sum(counts.values())
    print(f"Wrote {total} rows from {len(counts)} tables to {argv[0]}")
    print("Restore it on a SQLite Pinepods server running the same version: Settings > Restore Server.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
