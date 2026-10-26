//! Durable test reservations use the same sender and deadline policy as doses.
use crate::{
    delivery,
    push::{Outcome, Subscription},
};
use serde::Deserialize;
use worker::{D1Database, Env};

#[derive(Deserialize)]
struct Claim {
    owner: String,
    generation: i32,
}

#[derive(Deserialize)]
struct Reservation {
    subscription_id: String,
    endpoint: String,
    p256dh: String,
    auth: String,
    probe_at: i64,
    attempt_count: i32,
    updated_at: i64,
}

fn now() -> worker::Result<i64> {
    i64::try_from(worker::Date::now().as_millis())
        .map_err(|_| worker::Error::RustError("notification clock invalid".into()))
}

pub async fn run(env: &Env) -> worker::Result<()> {
    let db = env.d1("DB")?;
    let clock = now()?;
    db.prepare("UPDATE push_probe SET state='expired',lease_until=NULL,last_error_class='deadline' WHERE state IN ('pending','sending') AND probe_at<?1")
        .bind(&[(clock-delivery::WINDOW).to_string().into()])?.run().await?;
    let subject = env.secret("VAPID_SUBJECT")?.to_string();
    let private = env.secret("VAPID_PRIVATE_KEY")?.to_string();
    for _ in 0..100 {
        let clock = now()?;
        let token = clock + delivery::LEASE;
        let Some(claim) = db.prepare("UPDATE push_probe SET state='sending',attempt_count=attempt_count+1,lease_until=?2 WHERE owner=(SELECT p.owner FROM push_probe p JOIN push_subscriptions s ON s.owner=p.owner AND s.endpoint=p.endpoint AND s.p256dh=p.p256dh AND s.auth=p.auth AND s.enabled=1 WHERE p.probe_at<=?1 AND p.probe_at>=?3 AND p.attempt_count<?4 AND ((p.state='pending' AND p.next_attempt_at<=?1) OR (p.state='sending' AND p.lease_until<=?1)) ORDER BY p.probe_at LIMIT 1) RETURNING owner,generation")
            .bind(&[clock.to_string().into(),token.to_string().into(),(clock-delivery::WINDOW).to_string().into(),delivery::ATTEMPTS.into()])?
            .first::<Claim>(None).await? else { break };
        let Some(probe) = db.prepare("SELECT s.id AS subscription_id,p.endpoint,p.p256dh,p.auth,p.probe_at,p.attempt_count,s.updated_at FROM push_probe p JOIN push_subscriptions s ON s.owner=p.owner AND s.endpoint=p.endpoint AND s.p256dh=p.p256dh AND s.auth=p.auth AND s.enabled=1 WHERE p.owner=?1 AND p.generation=?2 AND p.state='sending' AND p.lease_until=?3")
            .bind(&[claim.owner.as_str().into(),claim.generation.into(),token.to_string().into()])?
            .first::<Reservation>(None).await? else { continue };
        if now()? >= token {
            continue;
        }
        let subscription = Subscription {
            endpoint: probe.endpoint.clone(),
            p256dh: probe.p256dh.clone(),
            auth: probe.auth.clone(),
        };
        let tag = format!("test:{}:{}", probe.subscription_id, claim.generation);
        let result = delivery::deliver(
            &subscription,
            &tag,
            probe.probe_at + delivery::WINDOW,
            probe.attempt_count,
            &subject,
            &private,
        )
        .await?;
        if result.outcome == Some(Outcome::Expired) {
            expire(&db, &claim, &probe, token).await?;
        }
        db.prepare("UPDATE push_probe SET state=?4,next_attempt_at=?5,lease_until=NULL,last_error_class=?6 WHERE owner=?1 AND generation=?2 AND state='sending' AND lease_until=?3")
            .bind(&[claim.owner.as_str().into(),claim.generation.into(),token.to_string().into(),result.state.into(),result.next.to_string().into(),result.class.into()])?.run().await?;
        let status = match result.outcome {
            Some(Outcome::Rejected(status)) => Some(status),
            _ => None,
        };
        crate::notifications::log_push_result(result.class, status);
    }
    Ok(())
}

async fn expire(
    db: &D1Database,
    claim: &Claim,
    probe: &Reservation,
    token: i64,
) -> worker::Result<()> {
    db.prepare("UPDATE push_subscriptions SET enabled=0 WHERE endpoint=?1 AND owner=?2 AND p256dh=?3 AND auth=?4 AND updated_at=?5 AND EXISTS(SELECT 1 FROM push_probe WHERE owner=?2 AND generation=?6 AND state='sending' AND lease_until=?7)")
        .bind(&[probe.endpoint.as_str().into(),claim.owner.as_str().into(),probe.p256dh.as_str().into(),probe.auth.as_str().into(),probe.updated_at.to_string().into(),claim.generation.into(),token.to_string().into()])?.run().await?;
    Ok(())
}
