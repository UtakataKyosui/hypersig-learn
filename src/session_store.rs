//! A `tower-sessions` store that keeps records in D1, so every isolate reads
//! the same sessions. Rows are keyed by the SHA-256 of the session id: a copy
//! of the table holds no usable cookie value.
use crate::repository::Db;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest as _, Sha256};
use tower_sessions::{
    SessionStore,
    session::{Id, Record},
    session_store::{Error, Result},
};
use worker::send::SendFuture;

const INSERT: &str = "INSERT OR IGNORE INTO sessions(id,record,expires_at) VALUES(?1,?2,?3)";
const UPSERT: &str = "INSERT INTO sessions(id,record,expires_at) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET record=excluded.record,expires_at=excluded.expires_at";
const LOAD: &str = "SELECT record FROM sessions WHERE id=?1 AND expires_at>?2";
const REMOVE: &str = "DELETE FROM sessions WHERE id=?1";
const ATTEMPTS: usize = 4;

#[derive(Clone)]
pub struct D1SessionStore {
    db: Db,
}
impl std::fmt::Debug for D1SessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("D1SessionStore")
    }
}
impl D1SessionStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

fn key(id: &Id) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(id.to_string().as_bytes()))
}

#[expect(clippy::cast_precision_loss, reason = "instants are below 2^53")]
fn expires_at(record: &Record) -> f64 {
    (record.expiry_date.unix_timestamp() * 1000) as f64
}

/// Every failure is reported without its cause: it could carry record data.
fn backend(_: worker::Error) -> Error {
    Error::Backend("session storage failure".into())
}

fn encode(record: &Record) -> Result<String> {
    serde_json::to_string(record).map_err(|_| Error::Encode("session record".into()))
}

#[async_trait::async_trait]
impl SessionStore for D1SessionStore {
    /// Retries with a new id when the random id is already taken.
    async fn create(&self, record: &mut Record) -> Result<()> {
        for _ in 0..ATTEMPTS {
            let json = encode(record)?;
            let inserted = SendFuture::new(async {
                let result = self
                    .db
                    .prepare(INSERT)
                    .bind(&[
                        key(&record.id).into(),
                        json.into(),
                        expires_at(record).into(),
                    ])?
                    .run()
                    .await?;
                Ok(result.meta()?.and_then(|meta| meta.changes).unwrap_or(0) > 0)
            })
            .await
            .map_err(backend)?;
            if inserted {
                return Ok(());
            }
            record.id = Id::default();
        }
        Err(Error::Backend("session id collision".into()))
    }

    async fn save(&self, record: &Record) -> Result<()> {
        let json = encode(record)?;
        SendFuture::new(async {
            self.db
                .prepare(UPSERT)
                .bind(&[
                    key(&record.id).into(),
                    json.into(),
                    expires_at(record).into(),
                ])?
                .run()
                .await?;
            Ok(())
        })
        .await
        .map_err(backend)
    }

    async fn load(&self, id: &Id) -> Result<Option<Record>> {
        #[expect(clippy::cast_precision_loss, reason = "instants are below 2^53")]
        let now = crate::auth_store::now_ms() as f64;
        let json = SendFuture::new(async {
            self.db
                .prepare(LOAD)
                .bind(&[key(id).into(), now.into()])?
                .first::<String>(Some("record"))
                .await
        })
        .await
        .map_err(backend)?;
        json.map(|json| {
            serde_json::from_str(&json).map_err(|_| Error::Decode("session record".into()))
        })
        .transpose()
    }

    async fn delete(&self, id: &Id) -> Result<()> {
        SendFuture::new(async {
            self.db
                .prepare(REMOVE)
                .bind(&[key(id).into()])?
                .run()
                .await?;
            Ok(())
        })
        .await
        .map_err(backend)
    }
}

/// An in-memory store for tests of code that needs a `SessionStore`.
#[cfg(test)]
pub mod testing {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };
    use tower_sessions::{
        SessionStore,
        session::{Id, Record},
        session_store::Result,
    };

    #[derive(Clone, Debug, Default)]
    pub struct Memory(Arc<Mutex<HashMap<Id, Record>>>);
    #[async_trait::async_trait]
    impl SessionStore for Memory {
        async fn create(&self, record: &mut Record) -> Result<()> {
            self.save(record).await
        }
        async fn save(&self, record: &Record) -> Result<()> {
            self.0.lock().unwrap().insert(record.id, record.clone());
            Ok(())
        }
        async fn load(&self, id: &Id) -> Result<Option<Record>> {
            Ok(self.0.lock().unwrap().get(id).cloned())
        }
        async fn delete(&self, id: &Id) -> Result<()> {
            self.0.lock().unwrap().remove(id);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_hides_the_cookie_value_and_is_stable() {
        let id = Id(42);
        assert_eq!(key(&id), key(&Id(42)));
        assert_ne!(key(&id), key(&Id(43)));
        assert!(!key(&id).contains(&id.to_string()));
        assert_eq!(key(&id).len(), 43);
    }
}
