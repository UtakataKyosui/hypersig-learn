//! What the server keeps in a session. The browser holds only the session id;
//! the user, the authentication level and the revocation version live in the
//! store and change only through the functions below.
#![expect(dead_code, reason = "wired up by the login routes")]
use crate::auth_store::User;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use tower_sessions::{Expiry, Session, session};

const IDENTITY: &str = "identity";
/// A signed-in session ends this long after sign-in, whatever its activity.
pub const LIFETIME: Duration = Duration::hours(24);

/// How strongly the visitor was authenticated. Only the server raises it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Level {
    /// Verified by a Google ID token.
    Google,
    /// Verified by Google and by a further factor, such as a passkey.
    Additional,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub user_id: String,
    pub session_version: i64,
    pub level: Level,
}

/// Signs `user` in on a new session id, so an id known before sign-in is
/// dropped, and ends the session at a fixed time.
pub async fn sign_in(
    session: &Session,
    user: &User,
    now: OffsetDateTime,
) -> Result<(), session::Error> {
    session
        .insert(
            IDENTITY,
            Identity {
                user_id: user.id.clone(),
                session_version: user.session_version,
                level: Level::Google,
            },
        )
        .await?;
    session.set_expiry(Some(Expiry::AtDateTime(now + LIFETIME)));
    session.cycle_id().await
}

/// Raises the level on a new session id. The level is never a request value.
pub async fn promote(session: &Session) -> Result<(), session::Error> {
    if let Some(mut identity) = current(session).await {
        identity.level = Level::Additional;
        session.insert(IDENTITY, identity).await?;
        session.cycle_id().await?;
    }
    Ok(())
}

/// The identity the session claims, before the user's current state is checked.
pub async fn current(session: &Session) -> Option<Identity> {
    session.get(IDENTITY).await.ok().flatten()
}

/// Deletes the record in the store and empties the session.
pub async fn sign_out(session: &Session) -> Result<(), session::Error> {
    session.flush().await
}

/// Whether `user` still honours the identity: active, and not revoked by a
/// newer `session_version`.
pub fn honoured(identity: &Identity, user: &User) -> bool {
    user.active() && user.id == identity.user_id && user.session_version == identity.session_version
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, sync::Arc, sync::Mutex};
    use tower_sessions::{
        SessionStore,
        session::{Id, Record},
        session_store,
    };

    #[derive(Debug, Default)]
    struct Memory(Mutex<HashMap<Id, Record>>);
    #[async_trait::async_trait]
    impl SessionStore for Memory {
        async fn create(&self, record: &mut Record) -> session_store::Result<()> {
            self.save(record).await
        }
        async fn save(&self, record: &Record) -> session_store::Result<()> {
            self.0.lock().unwrap().insert(record.id, record.clone());
            Ok(())
        }
        async fn load(&self, id: &Id) -> session_store::Result<Option<Record>> {
            Ok(self.0.lock().unwrap().get(id).cloned())
        }
        async fn delete(&self, id: &Id) -> session_store::Result<()> {
            self.0.lock().unwrap().remove(id);
            Ok(())
        }
    }

    fn user(version: i64, status: &str) -> User {
        User {
            id: "user".into(),
            status: status.into(),
            session_version: version,
        }
    }
    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap()
    }

    #[test]
    fn sign_in_changes_the_id_and_sign_out_removes_the_record() {
        futures::executor::block_on(async {
            let store = Arc::new(Memory::default());
            let session = Session::new(None, store.clone(), None);
            session.insert("before", 1).await.unwrap();
            session.save().await.unwrap();
            let anonymous = session.id().unwrap();

            sign_in(&session, &user(3, "active"), now()).await.unwrap();
            session.save().await.unwrap();
            let signed_in = session.id().unwrap();
            assert_ne!(anonymous, signed_in);
            assert!(store.load(&anonymous).await.unwrap().is_none());
            let record = store.load(&signed_in).await.unwrap().unwrap();
            assert_eq!(record.expiry_date, now() + LIFETIME);
            assert_eq!(
                current(&session).await,
                Some(Identity {
                    user_id: "user".into(),
                    session_version: 3,
                    level: Level::Google
                })
            );

            sign_out(&session).await.unwrap();
            assert!(store.load(&signed_in).await.unwrap().is_none());
            assert_eq!(current(&session).await, None);
        });
    }

    #[test]
    fn only_the_server_raises_the_level_and_the_id_changes() {
        futures::executor::block_on(async {
            let store = Arc::new(Memory::default());
            let session = Session::new(None, store.clone(), None);
            sign_in(&session, &user(1, "active"), now()).await.unwrap();
            session.save().await.unwrap();
            let before = session.id().unwrap();
            promote(&session).await.unwrap();
            session.save().await.unwrap();
            assert_ne!(session.id().unwrap(), before);
            assert!(store.load(&before).await.unwrap().is_none());
            assert_eq!(current(&session).await.unwrap().level, Level::Additional);
            assert!(Level::Additional > Level::Google);
        });
    }

    #[test]
    fn disabled_or_revoked_users_are_not_honoured() {
        let identity = Identity {
            user_id: "user".into(),
            session_version: 2,
            level: Level::Google,
        };
        assert!(honoured(&identity, &user(2, "active")));
        assert!(!honoured(&identity, &user(2, "disabled")));
        assert!(!honoured(&identity, &user(3, "active")));
        assert!(!honoured(
            &identity,
            &User {
                id: "other".into(),
                ..user(2, "active")
            }
        ));
    }
}
