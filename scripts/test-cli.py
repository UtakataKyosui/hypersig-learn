"""Verify cf migration tooling with fake setup responses and disposable local D1."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from auth_fixture import LOCAL_DATABASE_ID, ROOT

UUID = "11111111-1111-4111-8111-111111111111"


class SetupTests(unittest.TestCase):
    def setup_d1(self, databases, failure=False):
        with tempfile.TemporaryDirectory() as work:
            command = Path(work) / "npx"
            log = Path(work) / "calls.jsonl"
            command.write_text("""#!/usr/bin/env python3
import json, os, sys
with open(os.environ['CALL_LOG'], 'a') as log:
    log.write(json.dumps(sys.argv[1:]) + '\\n')
if 'list' in sys.argv:
    if os.environ.get('FAIL_LOOKUP') == '1': sys.exit(1)
    print(os.environ['DATABASES'])
else:
    print(json.dumps({'uuid': os.environ['CREATED_ID']}))
""")
            command.chmod(0o700)
            result = subprocess.run(
                ["sh", "scripts/setup-d1.sh", "--dry-run"], cwd=ROOT,
                capture_output=True, text=True,
                env={**os.environ, "PATH": work + os.pathsep + os.environ["PATH"],
                     "CALL_LOG": str(log), "DATABASES": json.dumps(databases),
                     "FAIL_LOOKUP": "1" if failure else "0", "CREATED_ID": UUID},
            )
            return result, [json.loads(line) for line in log.read_text().splitlines()]

    def test_existing_database_is_reused(self):
        result, calls = self.setup_d1([{"name": "app", "uuid": UUID}])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), "app " + UUID)
        self.assertEqual(len(calls), 1)

    def test_missing_database_is_created(self):
        result, calls = self.setup_d1([])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls[1][-3:], ["create", "--name", "app"])

    def test_lookup_failure_never_creates_a_database(self):
        result, calls = self.setup_d1([], failure=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(calls), 1)


class LocalImportTests(unittest.TestCase):
    def test_import_preserves_schema_data_history_and_source_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory(prefix="hypersig-cli-test-") as work:
            def query(database, sql):
                result = subprocess.run(
                    ["npx", "--no-install", "cf", "d1", "raw", database,
                     "--local", "--persist-to", work, "--sql", sql],
                    cwd=ROOT, check=True, capture_output=True, text=True,
                )
                return json.loads(result.stdout)[0]["results"]["rows"]

            query("DB", """
                CREATE TABLE example(id TEXT PRIMARY KEY, value TEXT);
                INSERT INTO example VALUES('old','kept');
                CREATE TABLE d1_migrations(id INTEGER PRIMARY KEY, name TEXT, applied_at TEXT);
                INSERT INTO d1_migrations VALUES(1,'0001.sql','2026-10-01');
                CREATE TRIGGER block_delete BEFORE DELETE ON example
                BEGIN SELECT RAISE(ABORT, 'blocked'); END;
            """)
            command = ["python3", "scripts/import-local-d1.py"]
            env = {**os.environ, "PERSIST_DIR": work}
            result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            for database in ("DB", LOCAL_DATABASE_ID):
                self.assertEqual(query(database, "SELECT * FROM example"), [["old", "kept"]])
                self.assertEqual(query(database, "SELECT name FROM d1_migrations"), [["0001.sql"]])
            self.assertEqual(query(LOCAL_DATABASE_ID, "SELECT name FROM sqlite_schema WHERE type='trigger'"), [["block_delete"]])
            result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Refusing to overwrite", result.stderr)


if __name__ == "__main__":
    unittest.main()
