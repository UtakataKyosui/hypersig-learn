#!/usr/bin/env python3
"""Run the repository's actual SQL against SQLite, with independent connections."""
import json
from pathlib import Path
import re
import sqlite3
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
QUERIES = re.findall(r'"((?:SELECT|INSERT|UPDATE|DELETE)[^"\n]+)"', (ROOT / "src/repository.rs").read_text())


def sql(prefix):
    matches = sorted({query for query in QUERIES if query.startswith(prefix)})
    assert len(matches) == 1, (prefix, matches)
    return matches[0]


class Persistence(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.path = Path(self.directory.name) / "db.sqlite"
        self.db = self.connect()
        self.db.executescript((ROOT / "migrations/0001_medication.sql").read_text())
        self.save(0)

    def connect(self):
        db = sqlite3.connect(self.path)
        db.execute("PRAGMA foreign_keys=ON")
        return db

    def tearDown(self):
        self.db.close()
        self.directory.cleanup()

    def save(self, revision, owner="owner", invalid=False):
        with self.db:
            self.db.execute(sql("INSERT INTO medications"), ("med", owner, "薬", 1, 1000, json.dumps({"id": "med"}), revision))
            self.db.execute(sql("SELECT CASE WHEN changes()=1 THEN 1 ELSE json('revision"))
            self.db.execute(sql("DELETE FROM pause_periods"), ("med",))
            self.db.execute(sql("INSERT INTO schedules"), ("med", "2026-10-01", None, "daily", 1, 0))
            self.db.execute(sql("INSERT INTO dose_slots"), ("slot", "med", "08:00", 50, "錠", "after_meal", ""))
            self.db.execute(sql("SELECT CASE WHEN changes()=1 THEN 1 ELSE json('slot"))
            self.db.execute(sql("DELETE FROM dose_slots"), ("med", '["slot"]'))
            self.db.execute(sql("INSERT INTO pause_periods"), ("pause", "med", "2026-10-03", "2026-10-05"))
            if invalid:
                self.db.execute(sql("INSERT INTO pause_periods"), ("overlap", "med", "2026-10-05", "2026-10-06"))

    def seed_job(self):
        with self.db:
            self.db.execute("INSERT INTO notification_jobs VALUES('job','med','slot',1000,1,'pending',901000)")
            self.db.execute(sql("INSERT INTO push_subscriptions"), ("sub", "owner", "https://fcm.googleapis.com/a", "key", "auth", 1000))
            self.db.execute("INSERT INTO notification_deliveries(job_id,subscription_id,state) VALUES('job','sub','pending')")

    def test_independent_state_and_atomic_revision_cancellation(self):
        self.seed_job()
        self.save(1)
        other = self.connect()
        try:
            self.assertEqual(other.execute(sql("SELECT document,revision,updated_at FROM medications WHERE owner=?1 AND id"), ("owner", "med")).fetchone()[1], 2)
            self.assertIsNone(other.execute(sql("SELECT document,revision,updated_at FROM medications WHERE owner=?1 AND id"), ("other", "med")).fetchone())
            self.assertEqual(other.execute("SELECT state FROM notification_jobs").fetchone()[0], "cancelled")
            self.assertEqual(other.execute("SELECT state FROM notification_deliveries").fetchone()[0], "cancelled")
        finally:
            other.close()

    def test_stale_revision_owner_and_failure_roll_back(self):
        self.seed_job()
        for revision, owner, invalid in [(0, "owner", False), (1, "other", False), (1, "owner", True)]:
            with self.assertRaises(sqlite3.DatabaseError):
                self.save(revision, owner, invalid)
            self.assertEqual(self.db.execute("SELECT revision FROM medications").fetchone()[0], 1)
            self.assertEqual(self.db.execute("SELECT state FROM notification_jobs").fetchone()[0], "pending")
            self.assertEqual(self.db.execute("SELECT count(*) FROM pause_periods").fetchone()[0], 1)

    def test_subscription_reregistration_owner_and_disable(self):
        self.seed_job()
        query = sql("INSERT INTO push_subscriptions")
        with self.db:
            self.db.execute(query, ("sub", "owner", "https://fcm.googleapis.com/a", "new-key", "new-auth", 2000))
        with self.assertRaises(sqlite3.DatabaseError), self.db:
            self.db.execute(query, ("sub", "other", "https://fcm.googleapis.com/a", "stolen", "stolen", 3000))
            self.db.execute(sql("SELECT CASE WHEN changes()=1 THEN 1 ELSE json('ownership"))
        with self.db:
            self.db.execute(sql("UPDATE push_subscriptions SET enabled=0,updated_at"), ("https://fcm.googleapis.com/a", "other", 3000))
            self.assertEqual(self.db.execute("SELECT changes()").fetchone()[0], 0)
            self.db.execute(sql("UPDATE push_subscriptions SET enabled=0,updated_at"), ("https://fcm.googleapis.com/a", "owner", 3000))
        other = self.connect()
        try:
            self.assertEqual(other.execute("SELECT enabled,p256dh FROM push_subscriptions").fetchone(), (0, "new-key"))
            self.assertEqual(other.execute("SELECT state FROM notification_deliveries").fetchone()[0], "cancelled")
        finally:
            other.close()

    def test_foreign_keys_unique_jobs_and_delete(self):
        self.seed_job()
        with self.assertRaises(sqlite3.IntegrityError), self.db:
            self.db.execute("INSERT INTO notification_jobs VALUES('duplicate','med','slot',1000,1,'pending',901000)")
        with self.assertRaises(sqlite3.IntegrityError), self.db:
            self.db.execute("INSERT INTO dose_slots VALUES('orphan','missing','08:00',50,'錠','after_meal','')")
        with self.db:
            self.db.execute(sql("DELETE FROM medications"), ("med", "other", 1))
            self.assertEqual(self.db.execute("SELECT changes()").fetchone()[0], 0)
            self.db.execute(sql("DELETE FROM medications"), ("med", "owner", 1))
        for table in ["medications", "schedules", "dose_slots", "pause_periods", "notification_jobs", "notification_deliveries"]:
            self.assertEqual(self.db.execute(f"SELECT count(*) FROM {table}").fetchone()[0], 0)


if __name__ == "__main__":
    unittest.main()
