DROP TRIGGER probe_subscription_disabled;
DROP INDEX probe_due;
-- The old sender uses only probe_at; do not replay completed reservations.
UPDATE push_probe SET probe_at=NULL WHERE state NOT IN ('pending','sending');
ALTER TABLE push_probe DROP COLUMN last_error_class;
ALTER TABLE push_probe DROP COLUMN lease_until;
ALTER TABLE push_probe DROP COLUMN next_attempt_at;
ALTER TABLE push_probe DROP COLUMN attempt_count;
ALTER TABLE push_probe DROP COLUMN state;
ALTER TABLE push_probe DROP COLUMN generation;
DELETE FROM d1_migrations WHERE name='0005_probe_delivery.sql';
