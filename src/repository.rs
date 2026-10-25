//! D1 persistence. Batch failures roll back every mutation; no runtime DDL.
use crate::medications::{Delete, Editor, Error};
use serde::Deserialize;
use std::rc::Rc;
use worker::{
    D1Database,
    send::{SendFuture, SendWrapper},
};
const LIST: &str =
    "SELECT document,revision,updated_at FROM medications WHERE owner=?1 ORDER BY id";
const LOAD: &str = "SELECT document,revision,updated_at FROM medications WHERE owner=?1 AND id=?2";
const IDENTITY: &str = "SELECT owner,revision FROM medications WHERE id=?1";
const SAVE: &str = "INSERT INTO medications(id,owner,name,enabled,revision,created_at,updated_at,document) VALUES(?1,?2,?3,?4,1,?5,?5,?6) ON CONFLICT(id) DO UPDATE SET name=excluded.name,enabled=excluded.enabled,revision=medications.revision+1,updated_at=excluded.updated_at,document=excluded.document WHERE medications.owner=?2 AND medications.revision=?7";
const ASSERT_REVISION: &str =
    "SELECT CASE WHEN changes()=1 THEN 1 ELSE json('revision conflict') END";
const DELETE_PAUSES: &str = "DELETE FROM pause_periods WHERE medication_id=?1";
const UPSERT_SCHEDULE: &str = "INSERT INTO schedules VALUES(?1,?1,?2,?3,?4,?5,?6,'Asia/Tokyo') ON CONFLICT(id) DO UPDATE SET start_date=excluded.start_date,end_date=excluded.end_date,repeat_kind=excluded.repeat_kind,interval_days=excluded.interval_days,weekdays=excluded.weekdays";
const UPSERT_SLOT: &str = "INSERT INTO dose_slots VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET local_time=excluded.local_time,amount_hundredths=excluded.amount_hundredths,unit=excluded.unit,timing=excluded.timing,note=excluded.note WHERE dose_slots.schedule_id=excluded.schedule_id";
const ASSERT_SLOT: &str =
    "SELECT CASE WHEN changes()=1 THEN 1 ELSE json('slot ownership conflict') END";
const DELETE_SLOTS: &str =
    "DELETE FROM dose_slots WHERE schedule_id=?1 AND id NOT IN(SELECT value FROM json_each(?2))";
const INSERT_PAUSE: &str = "INSERT INTO pause_periods VALUES(?1,?2,?3,?4)";
const DELETE: &str = "DELETE FROM medications WHERE id=?1 AND owner=?2 AND revision=?3";
const UPSERT_SUBSCRIPTION: &str = "INSERT INTO push_subscriptions VALUES(?1,?2,?3,?4,?5,1,?6) ON CONFLICT(endpoint) DO UPDATE SET p256dh=excluded.p256dh,auth=excluded.auth,enabled=1,updated_at=excluded.updated_at WHERE push_subscriptions.owner=excluded.owner";
const ASSERT_OWNER: &str =
    "SELECT CASE WHEN changes()=1 THEN 1 ELSE json('ownership conflict') END";
const SAVE_PROBE: &str = "INSERT INTO push_probe(id,endpoint,p256dh,auth) VALUES(1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET endpoint=excluded.endpoint,p256dh=excluded.p256dh,auth=excluded.auth,probe_at=NULL";
const DISABLE_SUBSCRIPTION: &str =
    "UPDATE push_subscriptions SET enabled=0,updated_at=?3 WHERE endpoint=?1 AND owner=?2";
const DISABLE_PROBE: &str = "UPDATE push_probe SET probe_at=NULL WHERE endpoint IN(SELECT endpoint FROM push_subscriptions WHERE endpoint=?1 AND owner=?2)";

pub type Db = SendWrapper<Rc<D1Database>>;
#[derive(Deserialize)]
pub struct Row {
    pub document: String,
    pub revision: i32,
    pub updated_at: i64,
}
impl Row {
    pub fn editor(&self) -> Result<Editor, Error> {
        let mut editor: Editor = serde_json::from_str(&self.document).map_err(|_| Error::Store)?;
        editor.revision = self.revision;
        Ok(editor)
    }
}
pub async fn list(db: &Db, owner: &str) -> Result<Vec<Row>, Error> {
    SendFuture::new(async {
        Ok(db
            .prepare(LIST)
            .bind(&[owner.into()])?
            .all()
            .await?
            .results()?)
    })
    .await
}
pub async fn load(db: &Db, owner: &str, id: &str) -> Result<Row, Error> {
    SendFuture::new(async {
        db.prepare(LOAD)
            .bind(&[owner.into(), id.into()])?
            .first::<Row>(None)
            .await?
            .ok_or(Error::Missing)
    })
    .await
}
pub async fn save(db: &Db, owner: &str, editor: &Editor, now: i64) -> Result<(), Error> {
    let domain = editor.domain(now)?;
    let document = serde_json::to_string(editor).map_err(|_| Error::Invalid)?;
    SendFuture::new(async {
        let existing = db
            .prepare(IDENTITY)
            .bind(&[editor.id.as_str().into()])?
            .first::<Identity>(None)
            .await?;
        match existing {
            Some(row) if row.owner != owner => return Err(Error::Missing),
            Some(row) if row.revision != editor.revision => return Err(Error::Conflict),
            None if editor.revision != 0 => return Err(Error::Missing),
            _ => {}
        }
        let mut statements = vec![
            db.prepare(SAVE).bind(&[
                editor.id.as_str().into(),
                owner.into(),
                domain.name.into(),
                i32::from(editor.enabled).into(),
                now.to_string().into(),
                document.into(),
                editor.revision.into(),
            ])?,
            db.prepare(ASSERT_REVISION),
            db.prepare(DELETE_PAUSES)
                .bind(&[editor.id.as_str().into()])?,
        ];
        let m = &editor.medication;
        statements.push(db.prepare(UPSERT_SCHEDULE).bind(&[
            editor.id.as_str().into(),
            m.start.as_str().into(),
            if m.end.is_empty() {
                wasm_bindgen::JsValue::NULL
            } else {
                m.end.as_str().into()
            },
            m.repeat.as_str().into(),
            m.interval.max(1).into(),
            m.weekdays.into(),
        ])?);
        for (slot, parsed) in editor.slots.iter().zip(domain.slots) {
            statements.push(db.prepare(UPSERT_SLOT).bind(&[
                slot.id.as_str().into(),
                editor.id.as_str().into(),
                parsed.time.to_string().into(),
                parsed.amount.hundredths().into(),
                parsed.unit.to_string().into(),
                slot.input.timing.as_str().into(),
                slot.input.note.as_str().into(),
            ])?);
            statements.push(db.prepare(ASSERT_SLOT));
        }
        let slot_ids =
            serde_json::to_string(&editor.slots.iter().map(|s| &s.id).collect::<Vec<_>>())
                .map_err(|_| Error::Invalid)?;
        statements.push(
            db.prepare(DELETE_SLOTS)
                .bind(&[editor.id.as_str().into(), slot_ids.into()])?,
        );
        for pause in &editor.pauses {
            statements.push(db.prepare(INSERT_PAUSE).bind(&[
                pause.id.as_str().into(),
                editor.id.as_str().into(),
                pause.input.start.as_str().into(),
                pause.input.end.as_str().into(),
            ])?);
        }
        db.batch(statements)
            .await
            .map_err(|error| batch_error(&error))?;
        Ok(())
    })
    .await
}
#[derive(Deserialize)]
struct Identity {
    owner: String,
    revision: i32,
}
pub async fn delete(db: &Db, owner: &str, input: &Delete) -> Result<(), Error> {
    SendFuture::new(async {
        let result = db
            .prepare(DELETE)
            .bind(&[
                input.id.as_str().into(),
                owner.into(),
                input.revision.into(),
            ])?
            .run()
            .await?;
        if result.meta()?.and_then(|m| m.changes).unwrap_or(0) == 0 {
            return Err(Error::Conflict);
        }
        Ok(())
    })
    .await
}

/// Registration is idempotent for an endpoint, but never transfers ownership.
pub async fn subscribe(
    db: &Db,
    owner: &str,
    subscription: crate::push::Subscription,
    now: i64,
) -> Result<(), Error> {
    use base64::Engine as _;
    use sha2::{Digest as _, Sha256};
    let id = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(subscription.endpoint.as_bytes()));
    SendFuture::new(async {
        db.batch(vec![
            db.prepare(UPSERT_SUBSCRIPTION).bind(&[
                id.into(),
                owner.into(),
                subscription.endpoint.as_str().into(),
                subscription.p256dh.as_str().into(),
                subscription.auth.as_str().into(),
                now.to_string().into(),
            ])?,
            db.prepare(ASSERT_OWNER),
            db.prepare(SAVE_PROBE).bind(&[
                subscription.endpoint.into(),
                subscription.p256dh.into(),
                subscription.auth.into(),
            ])?,
        ])
        .await
        .map_err(|error| batch_error(&error))?;
        Ok(())
    })
    .await
}
pub async fn unsubscribe(db: &Db, owner: &str, endpoint: &str, now: i64) -> Result<(), Error> {
    SendFuture::new(async {
        db.batch(vec![
            db.prepare(DISABLE_SUBSCRIPTION).bind(&[
                endpoint.into(),
                owner.into(),
                now.to_string().into(),
            ])?,
            db.prepare(ASSERT_OWNER),
            db.prepare(DISABLE_PROBE)
                .bind(&[endpoint.into(), owner.into()])?,
        ])
        .await
        .map_err(|error| batch_error(&error))?;
        Ok(())
    })
    .await
}

/// Returns only status; subscription credentials never enter page signals.
pub async fn subscription_enabled(db: &Db, owner: &str, endpoint: &str) -> Result<bool, Error> {
    SendFuture::new(async {
        Ok(db
            .prepare("SELECT enabled FROM push_subscriptions WHERE owner=?1 AND endpoint=?2")
            .bind(&[owner.into(), endpoint.into()])?
            .first::<i32>(Some("enabled"))
            .await?
            .unwrap_or(0)
            == 1)
    })
    .await
}

// Constraint failures and the atomic revision assertion are safe, retryable conflicts.
// Infrastructure failures retain a distinct 500 response without exposing SQL or values.
fn batch_error(error: &worker::Error) -> Error {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("malformed json")
        || message.contains("constraint")
        || message.contains("overlapping pause")
    {
        Error::Conflict
    } else {
        Error::Store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse as _;
    #[test]
    fn database_outage_is_distinct_from_edit_conflict_and_never_exposes_cause() {
        let outage = batch_error(&worker::Error::RustError(
            "database unavailable: private medication and keys".into(),
        ));
        let response = outage.into_response();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body =
            futures::executor::block_on(axum::body::to_bytes(response.into_body(), 4096)).unwrap();
        assert!(
            !String::from_utf8(body.to_vec())
                .unwrap()
                .contains("private")
        );
        let conflict = batch_error(&worker::Error::RustError("D1_ERROR: malformed JSON".into()));
        assert_eq!(
            conflict.into_response().status(),
            axum::http::StatusCode::CONFLICT
        );
    }
}

pub async fn counter_saved(db: &Db) -> worker::Result<i32> {
    SendFuture::new(async {
        Ok(db
            .prepare("SELECT count FROM counter WHERE id=1")
            .first::<i32>(Some("count"))
            .await?
            .unwrap_or(0))
    })
    .await
}
pub async fn counter_save(db: &Db, count: i32) -> worker::Result<()> {
    SendFuture::new(async { db.prepare("INSERT INTO counter(id,count) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET count=excluded.count").bind(&[count.into()])?.run().await?; Ok(()) }).await
}
pub async fn schedule_probe(db: &Db, at_ms: u64) -> worker::Result<bool> {
    SendFuture::new(async { let result=db.prepare("UPDATE push_probe SET probe_at=?1 WHERE id=1 AND endpoint IN(SELECT endpoint FROM push_subscriptions WHERE owner='owner' AND enabled=1)").bind(&[at_ms.to_string().into()])?.run().await?; Ok(result.meta()?.and_then(|m|m.changes).unwrap_or(0)>0) }).await
}
#[derive(Deserialize)]
struct ProbeRow {
    endpoint: String,
    p256dh: String,
    auth: String,
    probe_at: f64,
}
pub async fn claim_probe(
    db: &D1Database,
    now: u64,
    deadline: u64,
) -> worker::Result<Option<(crate::push::Subscription, f64)>> {
    let Some(row)=db.prepare("SELECT endpoint,p256dh,auth,probe_at FROM push_probe WHERE id=1 AND probe_at<=?1 AND probe_at>=?2 AND endpoint IN(SELECT endpoint FROM push_subscriptions WHERE owner='owner' AND enabled=1)").bind(&[now.to_string().into(),now.saturating_sub(deadline).to_string().into()])?.first::<ProbeRow>(None).await? else { return Ok(None); };
    let claimed = db
        .prepare("UPDATE push_probe SET probe_at=NULL WHERE id=1 AND probe_at=?1")
        .bind(&[row.probe_at.into()])?
        .run()
        .await?;
    if claimed.meta()?.and_then(|m| m.changes).unwrap_or(0) == 0 {
        return Ok(None);
    }
    Ok(Some((
        crate::push::Subscription {
            endpoint: row.endpoint,
            p256dh: row.p256dh,
            auth: row.auth,
        },
        row.probe_at,
    )))
}
pub async fn expire_subscription(
    db: &D1Database,
    subscription: &crate::push::Subscription,
) -> worker::Result<()> {
    db.batch(vec![
        db.prepare(
            "UPDATE push_subscriptions SET enabled=0 WHERE endpoint=?1 AND p256dh=?2 AND auth=?3",
        )
        .bind(&[
            subscription.endpoint.as_str().into(),
            subscription.p256dh.as_str().into(),
            subscription.auth.as_str().into(),
        ])?,
        db.prepare("DELETE FROM push_probe WHERE id=1 AND endpoint=?1 AND p256dh=?2 AND auth=?3")
            .bind(&[
                subscription.endpoint.as_str().into(),
                subscription.p256dh.as_str().into(),
                subscription.auth.as_str().into(),
            ])?,
    ])
    .await?;
    Ok(())
}
/// Puts the claimed probe time back, unless the subscription was replaced or
/// a newer probe was reserved while the send was in flight.
pub async fn restore_claim(db: &D1Database, endpoint: &str, probe_at: f64) -> worker::Result<()> {
    db.prepare(
        "UPDATE push_probe SET probe_at = ?1 WHERE id = 1 AND probe_at IS NULL AND endpoint = ?2",
    )
    .bind(&[probe_at.into(), endpoint.into()])?
    .run()
    .await?;
    Ok(())
}
