-- Keep one test reservation per owner, and fence in-flight responses by
-- generation as well as lease. Existing pending probes remain deliverable.
ALTER TABLE push_probe ADD COLUMN generation INTEGER NOT NULL DEFAULT 1;
ALTER TABLE push_probe ADD COLUMN state TEXT NOT NULL DEFAULT 'pending'
 CHECK(state IN ('pending','sending','sent','cancelled','expired'));
ALTER TABLE push_probe ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count>=0);
ALTER TABLE push_probe ADD COLUMN next_attempt_at INTEGER;
ALTER TABLE push_probe ADD COLUMN lease_until INTEGER;
ALTER TABLE push_probe ADD COLUMN last_error_class TEXT;
UPDATE push_probe SET next_attempt_at=probe_at;
CREATE INDEX probe_due ON push_probe(state,next_attempt_at,lease_until);
CREATE TRIGGER probe_subscription_disabled AFTER UPDATE OF enabled ON push_subscriptions WHEN NEW.enabled=0 BEGIN
 UPDATE push_probe SET state='cancelled',generation=generation+1,lease_until=NULL
 WHERE owner=NEW.owner AND endpoint=NEW.endpoint AND state IN ('pending','sending');
END;
