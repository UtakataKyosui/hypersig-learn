-- Reverts 0003_owner_isolation.sql. Run it with `wrangler d1 execute` only after
-- removing the deployed Worker version that reads these tables, and after
-- `scripts/legacy-owner.sh rollback` for every user it was run for.
DROP TABLE IF EXISTS legacy_owner_migration;
DROP TRIGGER IF EXISTS delivery_owner;
DROP TABLE IF EXISTS counters;
CREATE TABLE push_probe_single (
 id INTEGER PRIMARY KEY CHECK(id=1), endpoint TEXT NOT NULL,
 p256dh TEXT NOT NULL, auth TEXT NOT NULL, probe_at INTEGER
);
INSERT INTO push_probe_single(id,endpoint,p256dh,auth,probe_at) SELECT 1,endpoint,p256dh,auth,probe_at FROM push_probe WHERE owner='owner';
DROP TABLE push_probe;
ALTER TABLE push_probe_single RENAME TO push_probe;
DELETE FROM d1_migrations WHERE name='0003_owner_isolation.sql';
