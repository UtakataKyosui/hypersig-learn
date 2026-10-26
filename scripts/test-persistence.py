#!/usr/bin/env python3
"""Run the repository's actual SQL against SQLite, with independent connections."""
import json
from pathlib import Path
import re
import sqlite3
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
QUERIES = [
    query
    for source in ["src/repository.rs", "src/auth_store.rs", "src/session_store.rs", "src/delivery.rs", "src/probe_delivery.rs"]
    for query in re.findall(r'"((?:SELECT|INSERT|UPDATE|DELETE)[^"\n]+)"', (ROOT / source).read_text())
]


def sql(prefix):
    matches = sorted({query for query in QUERIES if query.startswith(prefix)})
    assert len(matches) == 1, (prefix, matches)
    return matches[0]


class Persistence(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.path = Path(self.directory.name) / "db.sqlite"
        self.db = self.connect()
        for migration in sorted((ROOT / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
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

    def test_expiry_ignores_replaced_keys_and_cancels_current_subscription(self):
        self.seed_job()
        endpoint = "https://fcm.googleapis.com/a"
        with self.db:
            self.db.execute(sql("INSERT INTO push_subscriptions"),
                            ("sub", "owner", endpoint, "new-key", "new-auth", 2000))
            self.db.execute(sql("INSERT INTO push_probe(owner,endpoint,p256dh,auth,probe_at"), (3000, "owner", endpoint))
            self.db.execute(sql("UPDATE push_probe SET state='sending',attempt_count"), (3000, 123000, -897000, 6))
            expire = sql("UPDATE push_subscriptions SET enabled=0 WHERE endpoint")
            self.db.execute(expire, (endpoint, "owner", "key", "auth", 1000, 1, 123000))
            self.assertEqual(self.db.execute("SELECT enabled FROM push_subscriptions").fetchone()[0], 1)
            self.assertEqual(self.db.execute("SELECT count(*) FROM push_probe").fetchone()[0], 1)
            self.db.execute(expire, (endpoint, "owner", "new-key", "new-auth", 2000, 1, 123000))
        other = self.connect()
        try:
            self.assertEqual(other.execute("SELECT enabled FROM push_subscriptions").fetchone()[0], 0)
            self.assertEqual(other.execute("SELECT state FROM push_probe").fetchone()[0], "cancelled")
            self.assertEqual(other.execute("SELECT state FROM notification_deliveries").fetchone()[0], "cancelled")
        finally:
            other.close()


class Accounts(unittest.TestCase):
    """Migration 0002 and the SQL of src/auth_store.rs, on independent connections."""

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.path = Path(self.directory.name) / "db.sqlite"
        self.db = self.connect()
        self.db.executescript((ROOT / "migrations/0001_medication.sql").read_text())
        with self.db:
            self.db.execute("INSERT INTO medications VALUES('med','owner','薬',1,1,1000,1000,'{}')")
            self.db.execute(sql("INSERT INTO push_subscriptions"), ("sub", "owner", "https://fcm.googleapis.com/a", "k", "a", 1000))
            self.db.execute("INSERT INTO push_probe VALUES(1,'https://fcm.googleapis.com/a','k','a',5000)")
        self.db.executescript((ROOT / "migrations/0002_auth.sql").read_text())
        self.db.executescript((ROOT / "migrations/0003_owner_isolation.sql").read_text())
        self.db.executescript((ROOT / "migrations/0004_flow_limit.sql").read_text())
        self.db.executescript((ROOT / "migrations/0005_probe_delivery.sql").read_text())

    def connect(self):
        db = sqlite3.connect(self.path)
        db.execute("PRAGMA foreign_keys=ON")
        return db

    def tearDown(self):
        self.db.close()
        self.directory.cleanup()

    def login(self, db, user_id, issuer="https://accounts.google.com", subject="sub-1", now=1000):
        with db:
            db.execute(sql("INSERT INTO users"), (user_id, now, issuer, subject))
            db.execute(sql("INSERT OR IGNORE INTO external_identities"), (user_id, now, issuer, subject))
            return db.execute(sql("SELECT u.id,u.status"), (issuer, subject)).fetchall()

    def test_migration_keeps_existing_data_and_down_script_reverts(self):
        self.assertEqual(self.db.execute("SELECT count(*) FROM medications").fetchone()[0], 1)
        self.assertEqual(self.db.execute("SELECT count(*) FROM push_subscriptions").fetchone()[0], 1)
        self.db.execute("CREATE TABLE d1_migrations(name TEXT)")
        self.db.execute("INSERT INTO d1_migrations VALUES('0002_auth.sql'),('0003_owner_isolation.sql'),('0004_flow_limit.sql'),('0005_probe_delivery.sql')")
        self.db.executescript((ROOT / "migrations/rollback/0005_probe_delivery_down.sql").read_text())
        self.db.executescript((ROOT / "migrations/rollback/0004_flow_limit_down.sql").read_text())
        self.assertNotIn("client", [row[1] for row in self.db.execute("PRAGMA table_info(oauth_flows)")])
        self.db.executescript((ROOT / "migrations/rollback/0003_owner_isolation_down.sql").read_text())
        self.assertEqual(self.db.execute("SELECT id,probe_at FROM push_probe").fetchall(), [(1, 5000)])
        self.db.executescript((ROOT / "migrations/rollback/0002_auth_down.sql").read_text())
        tables = {row[0] for row in self.db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        self.assertFalse(tables & {"users", "external_identities", "sessions", "oauth_flows"})
        self.assertEqual(self.db.execute("SELECT count(*) FROM medications").fetchone()[0], 1)
        self.assertEqual(self.db.execute("SELECT count(*) FROM d1_migrations").fetchone()[0], 0)

    def test_probe_downgrade_does_not_replay_terminal_reservations(self):
        with self.db:
            self.db.execute("CREATE TABLE d1_migrations(name TEXT)")
            self.db.execute("INSERT INTO d1_migrations VALUES('0005_probe_delivery.sql')")
            self.db.execute("UPDATE push_probe SET state='sent'")
        self.db.executescript((ROOT / "migrations/rollback/0005_probe_delivery_down.sql").read_text())
        self.assertEqual(self.db.execute("SELECT probe_at FROM push_probe").fetchone(), (None,))
        self.assertNotIn("generation", [row[1] for row in self.db.execute("PRAGMA table_info(push_probe)")])
        self.assertEqual(self.db.execute("SELECT count(*) FROM d1_migrations").fetchone()[0], 0)

    def test_first_logins_create_one_user_visible_to_other_connections(self):
        other = self.connect()
        try:
            first = self.login(self.db, "user-a")
            second = self.login(other, "user-b")
            self.assertEqual(first, [("user-a", "active", 1)])
            self.assertEqual(second, [("user-a", "active", 1)])
            self.assertEqual(self.db.execute("SELECT count(*) FROM users").fetchone()[0], 1)
            third = self.login(other, "user-c", subject="sub-2")
            self.assertEqual(third, [("user-c", "active", 1)])
            self.assertEqual(other.execute("SELECT count(*) FROM external_identities").fetchone()[0], 2)
        finally:
            other.close()

    def test_identity_is_keyed_by_issuer_and_subject_and_users_are_not_deleted_under_it(self):
        self.login(self.db, "user-a")
        self.assertEqual(self.login(self.db, "user-b", issuer="https://other.example")[0][0], "user-b")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO external_identities VALUES('https://accounts.google.com','sub-1','user-b',1)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO external_identities VALUES('i','s','missing',1)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("DELETE FROM users WHERE id='user-a'")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO users VALUES('x','unknown',1,1)")

    def test_flow_is_consumed_once_and_only_with_its_binding_before_expiry(self):
        with self.db:
            self.db.execute(sql("INSERT INTO oauth_flows"), ("reservation", "binding", 5000, "client", 1000, 10))
            self.db.execute(sql("UPDATE oauth_flows SET state"), ("state", "nonce", "verifier", "reservation"))
        other = self.connect()
        try:
            consume = sql("DELETE FROM oauth_flows WHERE state")

            def attempt(db, *args):
                with db:
                    return db.execute(consume, args).fetchall()

            self.assertEqual(attempt(other, "state", "wrong", 1000), [])
            self.assertEqual(attempt(other, "state", "binding", 5000), [])
            self.assertEqual(attempt(other, "state", "binding", 4999), [("nonce", "verifier")])
            self.assertEqual(attempt(self.db, "state", "binding", 1000), [])
        finally:
            other.close()

    def test_a_client_holds_a_bounded_number_of_live_flows(self):
        reserve = sql("INSERT INTO oauth_flows")

        def start(db, name, client, now, limit=2):
            with db:
                return db.execute(reserve, (name, "binding", now + 600, client, now, limit)).fetchall()

        other = self.connect()
        try:
            self.assertEqual(start(self.db, "a", "client-1", 1000), [("a",)])
            self.assertEqual(start(other, "b", "client-1", 1000), [("b",)])
            self.assertEqual(start(self.db, "c", "client-1", 1000), [])
            self.assertEqual(start(other, "d", "client-2", 1000), [("d",)])
            # A reservation that no callback filled still counts until it expires.
            self.assertEqual(start(self.db, "e", "client-1", 1599), [])
            self.assertEqual(start(self.db, "f", "client-1", 1600), [("f",)])
            self.assertEqual(self.db.execute("SELECT count(*) FROM oauth_flows WHERE client='client-1'").fetchone()[0], 3)
        finally:
            other.close()

    def test_purge_removes_only_expired_records(self):
        with self.db:
            self.db.execute("INSERT INTO sessions VALUES('old','{}',1000),('new','{}',9000)")
            self.db.execute(sql("INSERT INTO oauth_flows"), ("s1", "b", 1000, "c", 0, 10))
            self.db.execute(sql("INSERT INTO oauth_flows"), ("s2", "b", 9000, "c", 0, 10))
            self.db.execute(sql("DELETE FROM sessions WHERE expires_at"), (1000,))
            self.db.execute(sql("DELETE FROM oauth_flows WHERE expires_at"), (1000,))
        self.assertEqual(self.db.execute("SELECT id FROM sessions").fetchall(), [("new",)])
        self.assertEqual(self.db.execute("SELECT state FROM oauth_flows").fetchall(), [("s2",)])

    def test_session_store_statements(self):
        insert, upsert = sql("INSERT OR IGNORE INTO sessions"), sql("INSERT INTO sessions")
        load, remove = sql("SELECT record FROM sessions"), sql("DELETE FROM sessions WHERE id")
        other = self.connect()
        try:
            with self.db:
                self.assertEqual(self.db.execute(insert, ("a", '{"v":1}', 5000)).rowcount, 1)
            with other:
                # An id collision inserts nothing, so the store retries with a new id.
                self.assertEqual(other.execute(insert, ("a", '{"v":2}', 9000)).rowcount, 0)
            self.assertEqual(self.db.execute(load, ("a", 4999)).fetchall(), [('{"v":1}',)])
            self.assertEqual(other.execute(load, ("a", 5000)).fetchall(), [])
            with other:
                other.execute(upsert, ("a", '{"v":3}', 9000))
            self.assertEqual(self.db.execute(load, ("a", 6000)).fetchall(), [('{"v":3}',)])
            with other:
                other.execute(remove, ("a",))
            self.assertEqual(self.db.execute(load, ("a", 0)).fetchall(), [])
            with self.assertRaises(sqlite3.IntegrityError):
                self.db.execute(upsert, ("b", "not json", 1))
        finally:
            other.close()

    def test_existing_probe_becomes_the_legacy_owners_and_probes_are_per_owner(self):
        self.assertEqual(self.db.execute("SELECT owner,probe_at FROM push_probe").fetchall(), [("owner", 5000)])
        save, disable = sql("INSERT INTO push_probe(owner,endpoint,p256dh,auth) VALUES"), sql("UPDATE push_probe SET probe_at=NULL")
        schedule = sql("INSERT INTO push_probe(owner,endpoint,p256dh,auth,probe_at")
        with self.db:
            self.db.execute(save, ("https://fcm.googleapis.com/b", "kb", "ab", "user-b"))
            self.db.execute(save, ("https://fcm.googleapis.com/c", "kc", "ac", "user-c"))
            # Registering again replaces only that owner's probe.
            self.db.execute(save, ("https://fcm.googleapis.com/c2", "kc2", "ac2", "user-c"))
            self.db.execute(sql("INSERT INTO push_subscriptions"), ("sb", "user-b", "https://fcm.googleapis.com/b", "kb", "ab", 1000))
            self.db.execute(schedule, (9000, "user-b", "https://fcm.googleapis.com/b"))
        rows = dict((owner, (endpoint, at)) for owner, endpoint, at in self.db.execute("SELECT owner,endpoint,probe_at FROM push_probe"))
        self.assertEqual(rows["user-b"], ("https://fcm.googleapis.com/b", 9000))
        self.assertEqual(rows["user-c"], ("https://fcm.googleapis.com/c2", None))
        self.assertEqual(rows["owner"][0], "https://fcm.googleapis.com/a")
        # A user cannot schedule or disable another user's probe, nor schedule one without an enabled subscription.
        with self.db:
            self.assertEqual(self.db.execute(schedule, (9500, "user-c", "https://fcm.googleapis.com/b")).rowcount, 0)
            self.assertEqual(self.db.execute(disable, ("https://fcm.googleapis.com/b", "user-c")).rowcount, 0)
            self.assertEqual(self.db.execute(disable, ("https://fcm.googleapis.com/b", "user-b")).rowcount, 1)

    def test_a_delivery_cannot_join_a_job_and_a_subscription_of_different_owners(self):
        with self.db:
            self.db.execute(sql("INSERT INTO push_subscriptions"), ("sub-b", "user-b", "https://fcm.googleapis.com/b", "k", "a", 1000))
            self.db.execute("INSERT INTO schedules VALUES('med','med','2026-10-01',NULL,'daily',1,0,'Asia/Tokyo')")
            self.db.execute("INSERT INTO dose_slots VALUES('slot','med','08:00',50,'錠','after_meal','')")
            self.db.execute("INSERT INTO notification_jobs VALUES('job','med','slot',1000,1,'pending',901000)")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "delivery owner mismatch"):
            self.db.execute("INSERT INTO notification_deliveries(job_id,subscription_id,state) VALUES('job','sub-b','pending')")
        with self.db:
            self.db.execute("INSERT INTO notification_deliveries(job_id,subscription_id,state) VALUES('job','sub','pending')")

    def legacy(self, name, user):
        self.db.executescript((ROOT / "scripts/legacy-owner" / f"{name}.sql").read_text().replace("@USER@", user))

    def owners(self, table):
        return dict(self.db.execute(f"SELECT id,owner FROM {table}").fetchall())

    def test_legacy_data_moves_only_to_an_active_named_user_and_rolls_back(self):
        with self.db:
            self.db.execute("INSERT INTO users VALUES('user-a','active',1,1),('gone','disabled',1,1)")
            self.db.execute("INSERT INTO medications VALUES('med-a','user-a','自分の薬',1,1,1,1,'{}')")
        # An unknown or disabled user receives nothing and nothing is recorded.
        for user in ["nobody", "gone"]:
            self.legacy("migrate", user)
            self.assertEqual(self.owners("medications"), {"med": "owner", "med-a": "user-a"})
            self.assertEqual(self.db.execute("SELECT count(*) FROM legacy_owner_migration").fetchone()[0], 0)
        self.legacy("migrate", "user-a")
        self.assertEqual(self.owners("medications"), {"med": "user-a", "med-a": "user-a"})
        self.assertEqual(self.owners("push_subscriptions"), {"sub": "user-a"})
        self.assertEqual(self.db.execute("SELECT table_name,row_id FROM legacy_owner_migration ORDER BY 1").fetchall(),
                         [("medications", "med"), ("push_subscriptions", "sub")])
        # Running it again moves nothing more, and the child rows still belong to the same medication.
        self.legacy("migrate", "user-a")
        self.assertEqual(self.db.execute("SELECT count(*) FROM legacy_owner_migration").fetchone()[0], 2)
        # Rolling back returns only the migrated rows, not the user's own.
        self.legacy("rollback", "user-a")
        self.assertEqual(self.owners("medications"), {"med": "owner", "med-a": "user-a"})
        self.assertEqual(self.owners("push_subscriptions"), {"sub": "owner"})
        self.assertEqual(self.db.execute("SELECT count(*) FROM legacy_owner_migration").fetchone()[0], 0)
        self.assertEqual(self.db.execute("SELECT count(*) FROM medications").fetchone()[0], 2)
        counts = self.db.execute((ROOT / "scripts/legacy-owner/status.sql").read_text().replace("@USER@", "user-a")).fetchall()
        self.assertIn(("medications", "owner", 1), counts)

    def test_counters_are_per_owner(self):
        counter = sql("INSERT INTO counters")
        with self.db:
            self.db.execute(counter, ("user-a", 3))
            self.db.execute(counter, ("user-b", 5))
            self.db.execute(counter, ("user-a", 4))
        self.assertEqual(dict(self.db.execute("SELECT owner,count FROM counters").fetchall()), {"user-a": 4, "user-b": 5})


class ScheduledDelivery(Persistence):
    def reserve_probe(self, at=1000, endpoint="https://fcm.googleapis.com/a", owner="owner"):
        with self.db:
            return self.db.execute(sql("INSERT INTO push_probe(owner,endpoint,p256dh,auth,probe_at"), (at, owner, endpoint)).rowcount

    def claim_probe(self, now, lease, connection=None):
        db = connection or self.db
        with db:
            return db.execute(sql("UPDATE push_probe SET state='sending',attempt_count"), (now, lease, now-900000, 6)).fetchone()

    def claim(self, now, lease, connection=None):
        db = connection or self.db
        with db:
            return db.execute(sql("UPDATE notification_deliveries SET state='sending',attempt_count"), (now, lease, 6)).fetchone()

    def test_duplicate_generation_and_owner_revision_guard(self):
        self.db.execute(sql("INSERT INTO push_subscriptions"), ("sub", "owner", "https://fcm.googleapis.com/a", "key", "auth", 1000))
        job = sql("INSERT OR IGNORE INTO notification_jobs SELECT")
        deliveries = sql("INSERT OR IGNORE INTO notification_deliveries(job_id")
        for _ in range(2):
            with self.db:
                self.db.execute(job, ("job", "slot", 1000, 901000, "med", "owner", 1))
                self.db.execute(deliveries, ("job", 1000))
        self.assertEqual(self.db.execute("SELECT count(*) FROM notification_jobs").fetchone()[0], 1)
        self.assertEqual(self.db.execute("SELECT count(*) FROM notification_deliveries").fetchone()[0], 1)
        # A read racing with an edit cannot insert the old revision.
        self.save(1)
        with self.db:
            self.db.execute(job, ("stale", "slot", 1000, 901000, "med", "owner", 1))
        self.assertFalse(self.db.execute("SELECT 1 FROM notification_jobs WHERE id='stale'").fetchall())

    def test_claim_lease_recovery_and_stale_completion(self):
        self.seed_job()
        with self.db:
            self.db.execute("UPDATE notification_deliveries SET next_attempt_at=1000")
        other = self.connect()
        try:
            self.assertEqual(self.claim(1000, 121000), ("job", "sub"))
            self.assertIsNone(self.claim(1000, 121000, other))
            self.assertIsNone(self.claim(120999, 240999, other))
            self.assertEqual(self.claim(121000, 241000, other), ("job", "sub"))
            finish = sql("UPDATE notification_deliveries SET state=?4")
            with self.db:
                self.db.execute(finish, ("job", "sub", 121000, "sent", 121000, "accepted"))
            self.assertEqual(self.db.execute("SELECT state,attempt_count FROM notification_deliveries").fetchone(), ("sending", 2))
            with self.db:
                self.db.execute(finish, ("job", "sub", 241000, "pending", 300000, "retry"))
            self.assertIsNone(self.claim(299999, 419999))
            self.assertEqual(self.claim(300000, 420000), ("job", "sub"))
        finally:
            other.close()

    def test_expiry_and_attempt_limit_prevent_replay(self):
        self.seed_job()
        with self.db:
            self.db.execute("UPDATE notification_deliveries SET next_attempt_at=1000,attempt_count=6")
        self.assertIsNone(self.claim(1000, 121000))
        with self.db:
            self.db.execute("UPDATE notification_deliveries SET attempt_count=0")
        self.assertIsNone(self.claim(901001, 1021001))
        with self.db:
            self.db.execute(sql("UPDATE notification_jobs SET state='expired'"), (901001,))
            self.db.execute(sql("UPDATE notification_deliveries SET state='expired',lease_until=NULL"))
        self.assertEqual(self.db.execute("SELECT state FROM notification_deliveries").fetchone()[0], "expired")

    def test_expired_response_cannot_disable_reregistered_subscription_or_new_lease(self):
        self.seed_job()
        with self.db:
            self.db.execute("UPDATE notification_deliveries SET next_attempt_at=1000")
        self.assertEqual(self.claim(1000, 121000), ("job", "sub"))
        expire = sql("UPDATE push_subscriptions SET enabled=0 WHERE id=?1 AND endpoint")
        old = ("sub", "https://fcm.googleapis.com/a", "key", "auth", 1000, "job", 121000)
        # Even identical endpoint/keys represent a new registration after save.
        with self.db:
            self.db.execute(sql("INSERT INTO push_subscriptions"), ("ignored-id", "owner", old[1], "key", "auth", 2000))
            self.db.execute(expire, old)
        self.assertEqual(self.db.execute("SELECT enabled FROM push_subscriptions").fetchone()[0], 1)
        self.assertEqual(self.claim(121000, 241000), ("job", "sub"))
        with self.db:
            self.db.execute(expire, (*old[:4], 2000, "job", 121000))
        self.assertEqual(self.db.execute("SELECT enabled FROM push_subscriptions").fetchone()[0], 1)
        # Only the current lease and current subscription snapshot may expire it.
        with self.db:
            self.db.execute(expire, (*old[:4], 2000, "job", 241000))
        self.assertEqual(self.db.execute("SELECT enabled FROM push_subscriptions").fetchone()[0], 0)
        self.assertEqual(self.db.execute("SELECT state FROM notification_deliveries").fetchone()[0], "cancelled")

    def test_probe_claim_recovery_and_reservation_generation_fence(self):
        self.seed_job()
        self.assertEqual(self.reserve_probe(), 1)
        other = self.connect()
        try:
            self.assertEqual(self.claim_probe(1000, 121000), ("owner", 1))
            self.assertIsNone(self.claim_probe(1000, 121000, other))
            self.assertIsNone(self.claim_probe(120999, 240999, other))
            self.assertEqual(self.claim_probe(121000, 241000, other), ("owner", 1))
            complete = sql("UPDATE push_probe SET state=?4")
            with self.db:
                self.db.execute(complete, ("owner", 1, 121000, "sent", 121000, "accepted"))
            self.assertEqual(self.db.execute("SELECT state,attempt_count FROM push_probe").fetchone(), ("sending", 2))
            # Even an identical reservation time is a new generation.
            self.reserve_probe()
            self.assertEqual(self.claim_probe(121000, 241000), ("owner", 2))
            with self.db:
                self.db.execute(complete, ("owner", 1, 241000, "sent", 121000, "accepted"))
                self.db.execute(sql("UPDATE push_subscriptions SET enabled=0 WHERE endpoint"),
                                ("https://fcm.googleapis.com/a", "owner", "key", "auth", 1000, 1, 241000))
            self.assertEqual(self.db.execute("SELECT state,attempt_count FROM push_probe").fetchone(), ("sending", 1))
            self.assertEqual(self.db.execute("SELECT enabled FROM push_subscriptions").fetchone()[0], 1)
            with self.db:
                self.db.execute(complete, ("owner", 2, 241000, "pending", 300000, "retry"))
            self.assertIsNone(self.claim_probe(299999, 419999))
            self.assertEqual(self.claim_probe(300000, 420000), ("owner", 2))
        finally:
            other.close()

    def test_probe_uses_requested_device_and_stops_after_deadline_or_disable(self):
        self.seed_job()
        with self.db:
            self.db.execute(sql("INSERT INTO push_subscriptions"), ("other-device", "owner", "https://fcm.googleapis.com/b", "kb", "ab", 1000))
        self.reserve_probe(endpoint="https://fcm.googleapis.com/b")
        self.reserve_probe(endpoint="https://fcm.googleapis.com/a")
        self.assertEqual(self.db.execute("SELECT endpoint,generation FROM push_probe").fetchone(), ("https://fcm.googleapis.com/a", 2))
        self.assertEqual(self.reserve_probe(owner="another"), 0)
        self.assertEqual(self.claim_probe(901000, 1021000), ("owner", 2))
        self.reserve_probe()
        self.assertIsNone(self.claim_probe(901001, 1021001))
        with self.db:
            self.db.execute(sql("UPDATE push_probe SET state='expired',lease_until=NULL"), (1001,))
        self.assertEqual(self.db.execute("SELECT state FROM push_probe").fetchone()[0], "expired")
        self.reserve_probe()
        with self.db:
            self.db.execute("UPDATE push_probe SET attempt_count=6")
        self.assertIsNone(self.claim_probe(1000, 121000))
        self.reserve_probe()
        self.claim_probe(1000, 121000)
        with self.db:
            self.db.execute(sql("UPDATE push_subscriptions SET enabled=0,updated_at"), ("https://fcm.googleapis.com/a", "owner", 2000))
        self.assertIsNone(self.claim_probe(121000, 241000))
        self.assertEqual(self.db.execute("SELECT state FROM push_probe").fetchone()[0], "cancelled")


if __name__ == "__main__":
    unittest.main()
