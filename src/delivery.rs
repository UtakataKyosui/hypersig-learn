//! Durable scheduled delivery. A lease token fences completion by an older worker.
use crate::{
    domain::UtcInstant,
    medications::Editor,
    push::{self, Outcome, Subscription},
};
use serde::Deserialize;
use worker::{D1Database, Env};

pub(crate) const WINDOW: i64 = 900_000;
pub(crate) const LEASE: i64 = 120_000;
pub(crate) const ATTEMPTS: i32 = 6;

#[derive(Deserialize)]
struct MedicationRow {
    owner: String,
    document: String,
    revision: i32,
    updated_at: i64,
}

#[derive(Deserialize)]
struct Delivery {
    job_id: String,
    subscription_id: String,
    attempt_count: i32,
    endpoint: String,
    p256dh: String,
    auth: String,
    expires_at: i64,
    scheduled_at_utc: i64,
    dose_slot_id: String,
    schedule_revision: i32,
    document: String,
    revision: i32,
    updated_at: i64,
    subscription_updated_at: i64,
}

fn invalid() -> worker::Error {
    worker::Error::RustError("notification record invalid".into())
}

/// Clock is supplied by the caller; all candidates use the screen's domain calculation.
pub async fn generate(db: &D1Database, now: i64) -> worker::Result<()> {
    let rows = db
        .prepare("SELECT owner,document,revision,updated_at FROM medications WHERE enabled=1")
        .all()
        .await?
        .results::<MedicationRow>()?;
    for row in rows {
        let editor: Editor = serde_json::from_str(&row.document).map_err(|_| invalid())?;
        let medication = editor.domain(row.updated_at).map_err(|_| invalid())?;
        for occurrence in medication.due(UtcInstant(now)).map_err(|_| invalid())? {
            let slot = occurrence.slot_id.to_string();
            let id = serde_json::to_string(&(&slot, occurrence.at.0, row.revision))
                .map_err(|_| invalid())?;
            db.batch(vec![
                db.prepare("INSERT OR IGNORE INTO notification_jobs SELECT ?1,id,?2,?3,revision,'pending',?4 FROM medications WHERE id=?5 AND owner=?6 AND revision=?7 AND enabled=1")
                    .bind(&[id.as_str().into(), slot.into(), occurrence.at.0.to_string().into(), (occurrence.at.0+WINDOW).to_string().into(), editor.id.as_str().into(), row.owner.as_str().into(), row.revision.into()])?,
                db.prepare("INSERT OR IGNORE INTO notification_deliveries(job_id,subscription_id,state,next_attempt_at) SELECT j.id,s.id,'pending',?2 FROM notification_jobs j JOIN medications m ON m.id=j.medication_id JOIN push_subscriptions s ON s.owner=m.owner AND s.enabled=1 WHERE j.id=?1 AND j.state='pending' AND m.revision=j.schedule_revision AND s.updated_at<=j.scheduled_at_utc")
                    .bind(&[id.into(), now.to_string().into()])?,
            ]).await?;
        }
    }
    Ok(())
}

pub async fn run(env: &Env) -> worker::Result<()> {
    let db = env.d1("DB")?;
    let now = i64::try_from(worker::Date::now().as_millis()).map_err(|_| invalid())?;
    generate(&db, now).await?;
    db.batch(vec![
        db.prepare("UPDATE notification_jobs SET state='expired' WHERE state='pending' AND expires_at<?1").bind(&[now.to_string().into()])?,
        db.prepare("UPDATE notification_deliveries SET state='expired',lease_until=NULL WHERE state IN ('pending','sending') AND job_id IN (SELECT id FROM notification_jobs WHERE state='expired')"),
    ]).await?;
    let private = env.secret("VAPID_PRIVATE_KEY")?.to_string();
    let subject = env.secret("VAPID_SUBJECT")?.to_string();
    // Bounded batch; subsequent cron invocations pick up the remainder.
    for _ in 0..100 {
        let now = i64::try_from(worker::Date::now().as_millis()).map_err(|_| invalid())?;
        let token = now + LEASE;
        let claimed = db.prepare("UPDATE notification_deliveries SET state='sending',attempt_count=attempt_count+1,lease_until=?2 WHERE (job_id,subscription_id)=(SELECT d.job_id,d.subscription_id FROM notification_deliveries d JOIN notification_jobs j ON j.id=d.job_id JOIN push_subscriptions s ON s.id=d.subscription_id WHERE j.state='pending' AND j.expires_at>=?1 AND s.enabled=1 AND d.attempt_count<?3 AND ((d.state='pending' AND d.next_attempt_at<=?1) OR (d.state='sending' AND d.lease_until<=?1)) ORDER BY j.scheduled_at_utc LIMIT 1) RETURNING job_id,subscription_id")
            .bind(&[now.to_string().into(),token.to_string().into(),ATTEMPTS.into()])?
            .first::<Claim>(None).await?;
        let Some(claim) = claimed else { break };
        let row = db.prepare("SELECT d.job_id,d.subscription_id,d.attempt_count,s.endpoint,s.p256dh,s.auth,s.updated_at AS subscription_updated_at,j.expires_at,j.scheduled_at_utc,j.dose_slot_id,j.schedule_revision,m.document,m.revision,m.updated_at FROM notification_deliveries d JOIN notification_jobs j ON j.id=d.job_id JOIN medications m ON m.id=j.medication_id JOIN push_subscriptions s ON s.id=d.subscription_id AND s.owner=m.owner WHERE d.job_id=?1 AND d.subscription_id=?2 AND d.state='sending' AND d.lease_until=?3 AND j.state='pending' AND s.enabled=1 AND m.enabled=1")
            .bind(&[claim.job_id.as_str().into(),claim.subscription_id.as_str().into(),token.to_string().into()])?
            .first::<Delivery>(None).await?;
        let Some(row) = row else { continue };
        let now = i64::try_from(worker::Date::now().as_millis()).map_err(|_| invalid())?;
        if now >= token {
            continue;
        }
        let editor: Editor = serde_json::from_str(&row.document).map_err(|_| invalid())?;
        let valid = row.revision == row.schedule_revision
            && editor
                .domain(row.updated_at)
                .map_err(|_| invalid())?
                .due(UtcInstant(now))
                .map_err(|_| invalid())?
                .iter()
                .any(|o| {
                    o.at.0 == row.scheduled_at_utc && o.slot_id.to_string() == row.dose_slot_id
                });
        if !valid {
            finish(&db, &row, token, "cancelled", now, "changed").await?;
            continue;
        }
        let subscription = Subscription {
            endpoint: row.endpoint.clone(),
            p256dh: row.p256dh.clone(),
            auth: row.auth.clone(),
        };
        let result = deliver(
            &subscription,
            &row.job_id,
            row.expires_at,
            row.attempt_count,
            &subject,
            &private,
        )
        .await?;
        if result.outcome == Some(Outcome::Expired) {
            expire_subscription(&db, &row, token).await?;
        }
        finish(&db, &row, token, result.state, result.next, result.class).await?;
    }
    Ok(())
}

pub(crate) struct Attempt {
    pub outcome: Option<Outcome>,
    pub state: &'static str,
    pub class: &'static str,
    pub next: i64,
}

/// The same payload, remaining TTL, transport, and completion policy for
/// scheduled medication notifications and the current device's test.
pub(crate) async fn deliver(
    subscription: &Subscription,
    tag: &str,
    expires: i64,
    attempt: i32,
    subject: &str,
    private: &str,
) -> worker::Result<Attempt> {
    let now = i64::try_from(worker::Date::now().as_millis()).map_err(|_| invalid())?;
    if now > expires {
        return Ok(Attempt {
            outcome: None,
            state: "expired",
            class: "deadline",
            next: now,
        });
    }
    let payload = serde_json::json!({"url":crate::INDEX.url(()).as_str(),"tag":tag}).to_string();
    let ttl = u32::try_from((expires - now) / 1000).map_err(|_| invalid())?;
    let outcome = push::send_with_ttl(
        subscription,
        payload.as_bytes(),
        subject,
        private,
        u64::try_from(now / 1000).map_err(|_| invalid())?,
        ttl,
    )
    .await
    .unwrap_or(Outcome::Retry);
    let completed = i64::try_from(worker::Date::now().as_millis()).map_err(|_| invalid())?;
    let (state, class, next) = completion(&outcome, attempt, completed, expires);
    Ok(Attempt {
        outcome: Some(outcome),
        state,
        class,
        next,
    })
}

// A delayed response must not disable a re-registered subscription, or apply
// an earlier attempt's failure after another worker reclaimed its lease.
async fn expire_subscription(db: &D1Database, row: &Delivery, token: i64) -> worker::Result<()> {
    db.prepare("UPDATE push_subscriptions SET enabled=0 WHERE id=?1 AND endpoint=?2 AND p256dh=?3 AND auth=?4 AND updated_at=?5 AND EXISTS(SELECT 1 FROM notification_deliveries WHERE job_id=?6 AND subscription_id=?1 AND state='sending' AND lease_until=?7)")
        .bind(&[row.subscription_id.as_str().into(),row.endpoint.as_str().into(),row.p256dh.as_str().into(),row.auth.as_str().into(),row.subscription_updated_at.to_string().into(),row.job_id.as_str().into(),token.to_string().into()])?.run().await?;
    Ok(())
}

fn completion(
    outcome: &Outcome,
    attempt: i32,
    now: i64,
    expires: i64,
) -> (&'static str, &'static str, i64) {
    match outcome {
        Outcome::Accepted => ("sent", "accepted", now),
        Outcome::Expired => ("cancelled", "subscription_expired", now),
        Outcome::Rejected(_) => ("cancelled", "rejected", now),
        Outcome::Retry if attempt >= ATTEMPTS => ("expired", "attempt_limit", now),
        Outcome::Retry => {
            let next = now.saturating_add(30_000_i64 << attempt.clamp(0, 5));
            if next > expires {
                ("expired", "deadline", now)
            } else {
                ("pending", "retry", next)
            }
        }
    }
}

#[derive(Deserialize)]
struct Claim {
    job_id: String,
    subscription_id: String,
}

async fn finish(
    db: &D1Database,
    row: &Delivery,
    token: i64,
    state: &str,
    next: i64,
    class: &str,
) -> worker::Result<()> {
    db.batch(vec![
        db.prepare("UPDATE notification_deliveries SET state=?4,next_attempt_at=?5,lease_until=NULL,last_error_class=?6 WHERE job_id=?1 AND subscription_id=?2 AND state='sending' AND lease_until=?3")
            .bind(&[row.job_id.as_str().into(),row.subscription_id.as_str().into(),token.to_string().into(),state.into(),next.to_string().into(),class.into()])?,
        db.prepare("UPDATE notification_jobs SET state='sent' WHERE id=?1 AND state='pending' AND EXISTS(SELECT 1 FROM notification_deliveries WHERE job_id=?1) AND NOT EXISTS(SELECT 1 FROM notification_deliveries WHERE job_id=?1 AND state!='sent')")
            .bind(&[row.job_id.as_str().into()])?,
    ]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_responses_back_off_from_completion_and_stop_at_deadline() {
        for status in [429, 500, 502, 503, 599] {
            let outcome = Outcome::from_status(status);
            assert_eq!(
                completion(&outcome, 1, 100_000, 900_000),
                ("pending", "retry", 160_000)
            );
            assert_eq!(
                completion(&outcome, 2, 100_000, 900_000),
                ("pending", "retry", 220_000)
            );
            assert_eq!(
                completion(&outcome, 1, 840_000, 900_000),
                ("pending", "retry", 900_000)
            );
            assert_eq!(
                completion(&outcome, 1, 840_001, 900_000),
                ("expired", "deadline", 840_001)
            );
            assert_eq!(
                completion(&outcome, 1, 900_001, 900_000),
                ("expired", "deadline", 900_001)
            );
            assert_eq!(
                completion(&outcome, ATTEMPTS, 100_000, 900_000),
                ("expired", "attempt_limit", 100_000)
            );
        }
    }

    #[test]
    fn final_outcomes_never_retry_even_when_the_response_arrives_after_expiry() {
        for now in [100_000, 900_001] {
            assert_eq!(
                completion(&Outcome::Accepted, 1, now, 900_000),
                ("sent", "accepted", now)
            );
            for status in [404, 410] {
                assert_eq!(
                    completion(&Outcome::from_status(status), 1, now, 900_000),
                    ("cancelled", "subscription_expired", now)
                );
            }
            for status in [400, 401, 403] {
                assert_eq!(
                    completion(&Outcome::from_status(status), 1, now, 900_000),
                    ("cancelled", "rejected", now)
                );
            }
        }
    }
}
