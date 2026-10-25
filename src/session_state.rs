//! What the server keeps in a session. The browser holds only the session id;
//! the user, the authentication level and the revocation version live in the
//! store and change only through the functions below.
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
    /// When the session ends, in Unix seconds. The store's own expiry is
    /// recomputed whenever a request saves the session, so this is the limit.
    pub expires_at: i64,
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
                expires_at: (now + LIFETIME).unix_timestamp(),
            },
        )
        .await?;
    session.set_expiry(Some(Expiry::AtDateTime(now + LIFETIME)));
    session.cycle_id().await
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "used once an additional factor such as a passkey exists"
    )
)]
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

/// Keeps the session's end at the sign-in deadline. Call it before a handler
/// changes a signed-in session: the layer would otherwise save it with a new
/// expiry counted from now.
pub fn pin_expiry(session: &Session, identity: &Identity) {
    if let Ok(deadline) = OffsetDateTime::from_unix_timestamp(identity.expires_at) {
        session.set_expiry(Some(Expiry::AtDateTime(deadline)));
    }
}

/// Whether `user` still honours the identity at `now` (Unix seconds): before
/// the deadline, active, and not revoked by a newer `session_version`.
pub fn honoured(identity: &Identity, user: &User, now: i64) -> bool {
    now < identity.expires_at
        && user.active()
        && user.id == identity.user_id
        && user.session_version == identity.session_version
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_store::testing::Memory;
    use std::sync::Arc;
    use tower_sessions::SessionStore as _;

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
                    level: Level::Google,
                    expires_at: (now() + LIFETIME).unix_timestamp(),
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
            expires_at: 1000,
        };
        assert!(honoured(&identity, &user(2, "active"), 999));
        assert!(!honoured(&identity, &user(2, "active"), 1000));
        assert!(!honoured(&identity, &user(2, "disabled"), 0));
        assert!(!honoured(&identity, &user(3, "active"), 0));
        assert!(!honoured(
            &identity,
            &User {
                id: "other".into(),
                ..user(2, "active")
            },
            0
        ));
    }

    #[test]
    fn a_save_after_sign_in_cannot_extend_the_deadline() {
        futures::executor::block_on(async {
            let store = Arc::new(Memory::default());
            // The layer builds each request's session with its own inactivity expiry.
            let layer_expiry = Some(Expiry::OnInactivity(Duration::minutes(15)));
            let first = Session::new(None, store.clone(), layer_expiry);
            sign_in(&first, &user(1, "active"), now()).await.unwrap();
            first.save().await.unwrap();
            let id = first.id().unwrap();

            let later = Session::new(Some(id), store.clone(), layer_expiry);
            let identity = current(&later).await.unwrap();
            pin_expiry(&later, &identity);
            later.insert("notice", "failed").await.unwrap();
            later.save().await.unwrap();
            let record = store.load(&id).await.unwrap().unwrap();
            assert_eq!(record.expiry_date, now() + LIFETIME);

            // Without the pin the record would be extended from the current time.
            let unpinned = Session::new(Some(id), store.clone(), layer_expiry);
            unpinned.insert("notice", "again").await.unwrap();
            unpinned.save().await.unwrap();
            let record = store.load(&id).await.unwrap().unwrap();
            assert_ne!(record.expiry_date, now() + LIFETIME);
        });
    }
}
