-- One probe and one counter per user, a delivery that cannot cross users,
-- and the record that lets the single-owner data be handed to a user and back.
CREATE TABLE push_probe_by_owner (
 owner TEXT PRIMARY KEY, endpoint TEXT NOT NULL, p256dh TEXT NOT NULL, auth TEXT NOT NULL, probe_at INTEGER
);
INSERT INTO push_probe_by_owner(owner,endpoint,p256dh,auth,probe_at) SELECT 'owner',endpoint,p256dh,auth,probe_at FROM push_probe;
DROP TABLE push_probe;
ALTER TABLE push_probe_by_owner RENAME TO push_probe;
CREATE TABLE counters (owner TEXT PRIMARY KEY, count INTEGER NOT NULL);
CREATE TRIGGER delivery_owner BEFORE INSERT ON notification_deliveries
WHEN (SELECT m.owner FROM notification_jobs j JOIN medications m ON m.id=j.medication_id WHERE j.id=NEW.job_id)
 IS NOT (SELECT owner FROM push_subscriptions WHERE id=NEW.subscription_id)
BEGIN SELECT RAISE(ABORT,'delivery owner mismatch'); END;
CREATE TABLE legacy_owner_migration (
 table_name TEXT NOT NULL CHECK(table_name IN ('medications','push_subscriptions')), row_id TEXT NOT NULL,
 new_owner TEXT NOT NULL REFERENCES users(id), migrated_at INTEGER NOT NULL, PRIMARY KEY(table_name,row_id)
);
