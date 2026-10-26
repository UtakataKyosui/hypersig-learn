"""Verify the fast fixture path against the actual migrated SQLite schema."""
import os
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

import auth_fixture


class FixtureTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.persist = Path(self.directory.name)
        self.folder = self.persist / "v3/d1/miniflare-D1DatabaseObject"
        self.folder.mkdir(parents=True)
        self.path = self.folder / "fixture.sqlite"
        with sqlite3.connect(self.path) as db:
            for migration in sorted((auth_fixture.ROOT / "migrations").glob("*.sql")):
                db.executescript(migration.read_text())
        auth_fixture.fixture_database.cache_clear()
        self.addCleanup(auth_fixture.fixture_database.cache_clear)
        self.env = patch.dict(os.environ, {"FIXTURE_SQLITE": "1"})
        self.env.start()
        self.addCleanup(self.env.stop)

    def test_seed_and_query_are_visible_to_another_connection(self):
        auth_fixture.add_user(self.persist, "fixture-user")
        cookie = auth_fixture.new_session(self.persist, "fixture-user")
        self.assertTrue(cookie.startswith("__Host-session="))
        with sqlite3.connect(self.path) as db:
            self.assertEqual(db.execute("SELECT COUNT(*) FROM sessions").fetchone()[0], 1)
        self.assertEqual(auth_fixture.query(self.persist, "SELECT id, NULL AS empty FROM users"),
                         [{"id": "fixture-user", "empty": None}])

    def test_failed_batch_rolls_back_and_foreign_keys_are_enforced(self):
        with self.assertRaises(sqlite3.IntegrityError):
            auth_fixture.execute(self.persist, """
                INSERT INTO users VALUES('rollback','active',1,1000);
                INSERT INTO schedules VALUES('bad','missing','2026-01-01',NULL,'daily',1,0,'Asia/Tokyo');
            """)
        self.assertEqual(auth_fixture.query(self.persist, "SELECT id FROM users"), [])

    def test_missing_or_ambiguous_schema_fails_instead_of_guessing(self):
        with self.assertRaises(RuntimeError):
            auth_fixture.fixture_database(str(self.persist / "missing"))
        with sqlite3.connect(self.folder / "other.sqlite") as db:
            for table in ("users", "sessions", "medications", "notification_jobs"):
                db.execute(f"CREATE TABLE {table}(id TEXT)")
        with self.assertRaises(RuntimeError):
            auth_fixture.fixture_database(str(self.persist))

    def test_cli_is_still_the_default(self):
        with patch.dict(os.environ, {"FIXTURE_SQLITE": ""}), patch("auth_fixture.subprocess.run") as run:
            auth_fixture.execute(self.persist, "SELECT 1")
            self.assertIn("--local", run.call_args.args[0])
            self.assertIn(auth_fixture.LOCAL_DATABASE_ID, run.call_args.args[0])


if __name__ == "__main__":
    unittest.main()
