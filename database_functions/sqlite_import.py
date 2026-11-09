"""Move a SQLite Pinepods server to PostgreSQL or MariaDB/MySQL.

Start the new server (PostgreSQL or MariaDB, on the same Pinepods version as the SQLite
one), then run this inside it, pointing at the SQLite server's database file or one of its
backups:

    pinepods-db-setup --import-sqlite /opt/pinepods/data/pinepods.db     (in the container)
    python -m database_functions.sqlite_import pinepods.db                (from the repo)

Everything in the new server's database is replaced by the SQLite server's data. Login
sessions aren't copied, so everyone signs in again.
"""

import json
import os
import sqlite3
import sys

HEADER = "-- Pinepods SQLite backup"
SKIP = {"sessions", "gpoddersessions", "schema_migrations"}
BATCH = 1000


def open_source(path: str) -> sqlite3.Connection:
    """The SQLite database at `path`: a database file, or a SQLite server backup."""
    with open(path, "rb") as f:
        head = f.read(len(HEADER))
    if head == HEADER.encode():
        db = sqlite3.connect(":memory:")
        with open(path, encoding="utf-8") as f:
            db.executescript(f.read())
        return db
    if head.startswith(b"SQLite format 3"):
        return sqlite3.connect(f"file:{os.path.abspath(path)}?mode=ro", uri=True)
    raise SystemExit(f"{path} is neither a Pinepods SQLite database nor a SQLite backup")


def source_tables(src: sqlite3.Connection) -> list:
    """Source tables, parents before the tables that reference them."""
    names = [r[0] for r in src.execute(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")]
    parents = {n: {r[2] for r in src.execute(f'PRAGMA foreign_key_list("{n}")')} - {n} for n in names}
    ordered, done = [], set()
    while len(ordered) < len(names):
        ready = [n for n in names if n not in done and parents[n] <= done]
        if not ready:  # a cycle; take the rest as they are
            ready = [n for n in names if n not in done]
        for n in ready:
            ordered.append(n)
            done.add(n)
    return ordered


def versions(cursor, table: str) -> set:
    cursor.execute(f"SELECT version FROM {table}")
    return {r[0] for r in cursor.fetchall()}


class Target:
    """The PostgreSQL or MariaDB/MySQL database being filled."""

    def __init__(self, conn, db_type: str):
        self.conn, self.pg = conn, db_type == "postgresql"
        self.cur = conn.cursor()
        q = (lambda n: f'"{n}"') if self.pg else (lambda n: f"`{n}`")
        self.quote = q
        if self.pg:
            self.cur.execute("SET TIME ZONE 'UTC'")
            self.cur.execute(
                "SELECT table_name FROM information_schema.tables "
                "WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'")
        else:
            self.cur.execute(
                "SELECT TABLE_NAME FROM information_schema.TABLES "
                "WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE'")
        self.tables = {}
        for (name,) in self.cur.fetchall():
            name = name.decode() if isinstance(name, (bytes, bytearray)) else name
            self.tables[name.lower()] = name

    def columns(self, table: str) -> dict:
        """{lowercase column: (name, type, max length)}"""
        if self.pg:
            self.cur.execute(
                "SELECT column_name, data_type, udt_name, character_maximum_length FROM information_schema.columns "
                "WHERE table_schema = current_schema() AND table_name = %s", (table,))
            rows = [(c, (u[1:] + "[]") if d == "ARRAY" else u, n) for c, d, u, n in self.cur.fetchall()]
        else:
            self.cur.execute(
                "SELECT COLUMN_NAME, DATA_TYPE, CHARACTER_MAXIMUM_LENGTH FROM information_schema.COLUMNS "
                "WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = %s", (table,))
            rows = self.cur.fetchall()
        out = {}
        for c, t, n in rows:
            c = c.decode() if isinstance(c, (bytes, bytearray)) else c
            t = t.decode() if isinstance(t, (bytes, bytearray)) else t
            out[c.lower()] = (c, t.lower(), n)
        return out

    def clear(self):
        names = [self.tables[t] for t in self.tables if t != "schema_migrations"]
        if self.pg:
            self.cur.execute("TRUNCATE " + ", ".join(self.quote(n) for n in names) + " RESTART IDENTITY CASCADE")
        else:
            self.cur.execute("SET FOREIGN_KEY_CHECKS = 0")
            for n in names:
                self.cur.execute(f"DELETE FROM {self.quote(n)}")

    def placeholder(self, ctype: str) -> str:
        return f"%s::{ctype}" if self.pg else "%s"

    def convert(self, v, ctype: str, max_len, note):
        """A SQLite value as the target column type expects it."""
        if v is None:
            return None
        if self.pg:
            if ctype == "bool":
                return v.strip().lower() in ("1", "t", "true") if isinstance(v, str) else bool(v)
            if ctype.endswith("[]"):
                return json.loads(v) if isinstance(v, str) else v
            if ctype in ("json", "jsonb"):
                return v if isinstance(v, str) else json.dumps(v)
            if ctype == "bytea":
                return bytes(v) if isinstance(v, (bytes, bytearray, memoryview)) else str(v).encode()
        if isinstance(v, str) and max_len and len(v) > max_len:
            note()
            return v[:max_len]
        if isinstance(v, (int, float)) and ctype in ("text", "varchar", "char", "character varying", "longtext", "mediumtext"):
            return str(v)
        return v

    def reset_ids(self):
        """Point id sequences past the copied rows (MariaDB/MySQL does this itself)."""
        if not self.pg:
            self.cur.execute("SET FOREIGN_KEY_CHECKS = 1")
            return
        self.cur.execute(
            "SELECT table_name, column_name FROM information_schema.columns WHERE table_schema = current_schema() "
            "AND (column_default LIKE 'nextval(%%' OR is_identity = 'YES')")
        for table, column in self.cur.fetchall():
            t, c = self.quote(table), self.quote(column)
            self.cur.execute(
                f"SELECT setval(pg_get_serial_sequence(%s, %s), COALESCE((SELECT MAX({c}) FROM {t}), 0) + 1, false)",
                (t, column))


def import_sqlite(src: sqlite3.Connection, conn, db_type: str, replace: bool = False) -> dict:
    """Copy every row of `src` into `conn`, replacing what's there; returns rows per table."""
    target = Target(conn, db_type)
    if "schema_migrations" not in target.tables:
        raise SystemExit("The target database has no schema yet; start the new Pinepods server once first.")
    have = {r[0] for r in src.execute("SELECT version FROM schema_migrations")}
    want = versions(target.cur, target.quote(target.tables["schema_migrations"]))
    if have - want:
        raise SystemExit(f"The SQLite database is from a newer Pinepods version (migrations {', '.join(sorted(have - want))}). "
                         "Run the same version on both servers.")
    if want - have:
        raise SystemExit(f"The SQLite database is from an older Pinepods version (missing migrations {', '.join(sorted(want - have))}). "
                         "Update the SQLite server to this version first, or run the same version on both servers.")
    if not replace and "podcasts" in target.tables:
        target.cur.execute(f"SELECT COUNT(*) FROM {target.quote(target.tables['podcasts'])}")
        if target.cur.fetchone()[0]:
            raise SystemExit("The target database already has podcasts. Everything in it would be replaced; "
                             "pass --replace to go ahead.")

    target.clear()
    counts, truncated = {}, {}
    for table in source_tables(src):
        if table.lower() in SKIP:
            continue
        name = target.tables.get(table.lower())
        if name is None:
            print(f"Skipping {table}: not in the target database", file=sys.stderr)
            continue
        tcols = target.columns(name)
        scur = src.execute(f'SELECT * FROM "{table}"')
        scols = [d[0] for d in scur.description]
        shared = [(i, tcols[c.lower()]) for i, c in enumerate(scols) if c.lower() in tcols]
        dropped = [c for c in scols if c.lower() not in tcols]
        if dropped:
            print(f"Skipping {table} columns not in the target: {', '.join(dropped)}", file=sys.stderr)
        sql = (f"INSERT INTO {target.quote(name)} ({', '.join(target.quote(c) for _, (c, _, _) in shared)}) "
               f"VALUES ({', '.join(target.placeholder(t) for _, (_, t, _) in shared)})")

        def note(table=table):
            truncated[table] = truncated.get(table, 0) + 1

        n = 0
        while True:
            rows = scur.fetchmany(BATCH)
            if not rows:
                break
            target.cur.executemany(sql, [
                tuple(target.convert(row[i], t, m, note) for i, (_, t, m) in shared) for row in rows])
            n += len(rows)
        counts[table] = n
    target.reset_ids()
    conn.commit()
    for table, k in truncated.items():
        print(f"Shortened {k} over-long value(s) in {table} to fit the target's column size", file=sys.stderr)
    return counts


def main(argv) -> int:
    args = [a for a in argv if a != "--replace"]
    if len(args) != 1:
        print("usage: python -m database_functions.sqlite_import <SQLite database or backup> [--replace]", file=sys.stderr)
        return 2
    from database_functions.migrations import get_migration_manager

    manager = get_migration_manager()
    if manager.db_type == "sqlite":
        print("Run this on the new PostgreSQL or MariaDB server, not the SQLite one.", file=sys.stderr)
        return 1
    # The target's schema has to be current before data goes in.
    if not manager.run_migrations():
        print("Database migrations failed; nothing was imported.", file=sys.stderr)
        return 1
    db_type = "postgresql" if manager.db_type in ("postgresql", "postgres") else "mysql"
    src = open_source(args[0])
    conn = manager.get_connection()
    try:
        counts = import_sqlite(src, conn, db_type, replace="--replace" in argv)
    except BaseException:
        conn.rollback()
        raise
    print(f"Imported {sum(counts.values())} rows from {len(counts)} tables.")
    print("Restart Pinepods, then sign in with your existing account.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
