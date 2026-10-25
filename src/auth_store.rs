//! D1 storage for users, external identities, sessions and OAuth flows.
use crate::repository::Db;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use wasm_bindgen::JsValue;
use worker::send::SendFuture;

const CREATE_FLOW: &str =
    "INSERT INTO oauth_flows(state,nonce,pkce_verifier,binding,expires_at) VALUES(?1,?2,?3,?4,?5)";
const CONSUME_FLOW: &str = "DELETE FROM oauth_flows WHERE state=?1 AND binding=?2 AND expires_at>?3 RETURNING nonce,pkce_verifier";
const INSERT_USER: &str = "INSERT INTO users(id,status,session_version,created_at) SELECT ?1,'active',1,?2 WHERE NOT EXISTS(SELECT 1 FROM external_identities WHERE issuer=?3 AND subject=?4)";
const INSERT_IDENTITY: &str = "INSERT OR IGNORE INTO external_identities(issuer,subject,user_id,created_at) SELECT ?3,?4,?1,?2 WHERE EXISTS(SELECT 1 FROM users WHERE id=?1)";
const FIND_USER: &str = "SELECT u.id,u.status,u.session_version FROM external_identities i JOIN users u ON u.id=i.user_id WHERE i.issuer=?1 AND i.subject=?2";
const LOAD_USER: &str = "SELECT id,status,session_version FROM users WHERE id=?1";
const PURGE_SESSIONS: &str = "DELETE FROM sessions WHERE expires_at<=?1";
const PURGE_FLOWS: &str = "DELETE FROM oauth_flows WHERE expires_at<=?1";

/// D1 binds a JavaScript number, and a `BigInt` is refused. Millisecond
/// instants stay far below 2^53.
#[expect(clippy::cast_precision_loss, reason = "instants are below 2^53")]
fn ms(value: i64) -> JsValue {
    JsValue::from_f64(value as f64)
}

/// A random URL-safe token of 256 bits, for ids, state, nonce and bindings.
pub fn random_token() -> worker::Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| worker::Error::RustError("random source".into()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[derive(Deserialize)]
pub struct Flow {
    pub nonce: String,
    pub pkce_verifier: String,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct User {
    pub id: String,
    pub status: String,
    pub session_version: i64,
}
impl User {
    pub fn active(&self) -> bool {
        self.status == "active"
    }
}

pub async fn create_flow(
    db: &Db,
    state: &str,
    nonce: &str,
    verifier: &str,
    binding: &str,
    expires_at: i64,
) -> worker::Result<()> {
    SendFuture::new(async {
        db.prepare(CREATE_FLOW)
            .bind(&[
                state.into(),
                nonce.into(),
                verifier.into(),
                binding.into(),
                ms(expires_at),
            ])?
            .run()
            .await?;
        Ok(())
    })
    .await
}

/// Deletes and returns the flow in one statement, so only one callback wins.
/// A wrong binding, an expired flow and a repeated state all return `None`.
pub async fn consume_flow(
    db: &Db,
    state: &str,
    binding: &str,
    now: i64,
) -> worker::Result<Option<Flow>> {
    SendFuture::new(async {
        db.prepare(CONSUME_FLOW)
            .bind(&[state.into(), binding.into(), ms(now)])?
            .first::<Flow>(None)
            .await
    })
    .await
}

/// The user for a verified `(issuer, subject)`, created on first sight. One
/// batch is one transaction, so concurrent first logins end with one user.
pub async fn login(
    db: &Db,
    issuer: &str,
    subject: &str,
    new_user_id: &str,
    now: i64,
) -> worker::Result<User> {
    SendFuture::new(async {
        let mut results = db
            .batch(vec![
                db.prepare(INSERT_USER).bind(&[
                    new_user_id.into(),
                    ms(now),
                    issuer.into(),
                    subject.into(),
                ])?,
                db.prepare(INSERT_IDENTITY).bind(&[
                    new_user_id.into(),
                    ms(now),
                    issuer.into(),
                    subject.into(),
                ])?,
                db.prepare(FIND_USER)
                    .bind(&[issuer.into(), subject.into()])?,
            ])
            .await?;
        results
            .pop()
            .and_then(|result| result.results::<User>().ok())
            .and_then(|mut users| users.pop())
            .ok_or_else(|| worker::Error::RustError("identity missing".into()))
    })
    .await
}

pub async fn user(db: &Db, id: &str) -> worker::Result<Option<User>> {
    SendFuture::new(async {
        db.prepare(LOAD_USER)
            .bind(&[id.into()])?
            .first::<User>(None)
            .await
    })
    .await
}

/// Removes expired sessions and flows. Expired records are also refused on read.
pub async fn purge_expired(db: &Db, now: i64) -> worker::Result<()> {
    SendFuture::new(async {
        db.batch(vec![
            db.prepare(PURGE_SESSIONS).bind(&[ms(now)])?,
            db.prepare(PURGE_FLOWS).bind(&[ms(now)])?,
        ])
        .await?;
        Ok(())
    })
    .await
}

/// The current time in Unix milliseconds. The worker's `Date` is used on
/// wasm, where `SystemTime` panics.
pub fn now_ms() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        i64::try_from(worker::Date::now().as_millis()).unwrap_or(i64::MAX)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after Unix epoch")
                .as_millis(),
        )
        .unwrap_or(i64::MAX)
    }
}
