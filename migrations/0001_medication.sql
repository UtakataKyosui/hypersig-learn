-- Dates: ISO Gregorian calendar; instants: Unix milliseconds; amounts: hundredths.
CREATE TABLE IF NOT EXISTS counter (id INTEGER PRIMARY KEY, count INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS push_probe (
 id INTEGER PRIMARY KEY CHECK(id=1), endpoint TEXT NOT NULL,
 p256dh TEXT NOT NULL, auth TEXT NOT NULL, probe_at INTEGER
);
CREATE TABLE medications (
 id TEXT PRIMARY KEY, owner TEXT NOT NULL, name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 100),
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1)), revision INTEGER NOT NULL CHECK(revision>0),
 created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, document TEXT NOT NULL CHECK(json_valid(document))
);
CREATE INDEX medications_owner ON medications(owner);
CREATE TABLE schedules (
 id TEXT PRIMARY KEY, medication_id TEXT NOT NULL UNIQUE REFERENCES medications(id) ON DELETE CASCADE,
 start_date TEXT NOT NULL, end_date TEXT, repeat_kind TEXT NOT NULL CHECK(repeat_kind IN ('daily','weekdays','interval')),
 interval_days INTEGER NOT NULL CHECK(interval_days>=1), weekdays INTEGER NOT NULL CHECK(weekdays BETWEEN 0 AND 127),
 timezone TEXT NOT NULL CHECK(timezone='Asia/Tokyo'), CHECK(end_date IS NULL OR end_date>=start_date),
 CHECK(repeat_kind!='weekdays' OR weekdays>0)
);
CREATE TABLE dose_slots (
 id TEXT PRIMARY KEY, schedule_id TEXT NOT NULL REFERENCES schedules(id) ON DELETE CASCADE,
 local_time TEXT NOT NULL, amount_hundredths INTEGER NOT NULL CHECK(amount_hundredths BETWEEN 1 AND 100000),
 unit TEXT NOT NULL CHECK(length(unit) BETWEEN 1 AND 20), timing TEXT NOT NULL, note TEXT NOT NULL CHECK(length(note)<=200)
);
CREATE INDEX slots_schedule ON dose_slots(schedule_id);
CREATE TABLE pause_periods (
 id TEXT PRIMARY KEY, medication_id TEXT NOT NULL REFERENCES medications(id) ON DELETE CASCADE,
 start_date TEXT NOT NULL, end_date TEXT NOT NULL CHECK(end_date>=start_date)
);
CREATE INDEX pauses_medication ON pause_periods(medication_id,start_date,end_date);
CREATE TRIGGER pause_overlap BEFORE INSERT ON pause_periods BEGIN
 SELECT RAISE(ABORT,'overlapping pause') WHERE EXISTS(
 SELECT 1 FROM pause_periods WHERE medication_id=NEW.medication_id AND start_date<=NEW.end_date AND end_date>=NEW.start_date);
END;
CREATE TABLE push_subscriptions (
 id TEXT PRIMARY KEY, owner TEXT NOT NULL, endpoint TEXT NOT NULL UNIQUE, p256dh TEXT NOT NULL, auth TEXT NOT NULL,
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1)), updated_at INTEGER NOT NULL
);
CREATE INDEX subscriptions_owner ON push_subscriptions(owner,enabled);
CREATE TABLE notification_jobs (
 id TEXT PRIMARY KEY, medication_id TEXT NOT NULL REFERENCES medications(id) ON DELETE CASCADE,
 dose_slot_id TEXT NOT NULL REFERENCES dose_slots(id) ON DELETE CASCADE, scheduled_at_utc INTEGER NOT NULL, schedule_revision INTEGER NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('pending','cancelled','expired','sent')), expires_at INTEGER NOT NULL,
 UNIQUE(dose_slot_id,scheduled_at_utc,schedule_revision), CHECK(expires_at>=scheduled_at_utc)
);
CREATE INDEX jobs_due ON notification_jobs(state,scheduled_at_utc,expires_at);
CREATE TABLE notification_deliveries (
 job_id TEXT NOT NULL REFERENCES notification_jobs(id) ON DELETE CASCADE,
 subscription_id TEXT NOT NULL REFERENCES push_subscriptions(id) ON DELETE CASCADE,
 state TEXT NOT NULL CHECK(state IN ('pending','sending','sent','cancelled','expired')),
 attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count>=0), next_attempt_at INTEGER,
 lease_until INTEGER, last_error_class TEXT, PRIMARY KEY(job_id,subscription_id)
);
CREATE INDEX deliveries_due ON notification_deliveries(state,next_attempt_at,lease_until);
CREATE TRIGGER medication_revision AFTER UPDATE OF revision ON medications BEGIN
 UPDATE notification_jobs SET state='cancelled' WHERE medication_id=NEW.id AND schedule_revision!=NEW.revision AND state='pending';
 UPDATE notification_deliveries SET state='cancelled' WHERE state IN ('pending','sending') AND job_id IN
 (SELECT id FROM notification_jobs WHERE medication_id=NEW.id AND state='cancelled');
END;
CREATE TRIGGER subscription_disabled AFTER UPDATE OF enabled ON push_subscriptions WHEN NEW.enabled=0 BEGIN
 UPDATE notification_deliveries SET state='cancelled' WHERE subscription_id=NEW.id AND state IN ('pending','sending');
END;
