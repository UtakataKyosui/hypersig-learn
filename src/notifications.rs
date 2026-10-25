//! The notification settings page and the verification of Web Push on a
//! worker: a browser subscribes, the server saves and checks the
//! subscription, and a scheduled invocation sends the probe notification.

use std::rc::Rc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use hypersig::html::{Class, CustomEvent, div, el, h1, main, p};
use hypersig::{Action, Page, Patches, Signals};
use hypersig_ui::{
    AlertVariant, ButtonSize, ButtonVariant, Level, alert, alert_description, alert_title, button,
    card, card_content, card_description, card_header, card_title,
};
use serde::{Deserialize, Serialize};
use worker::send::{SendFuture, SendWrapper};
use worker::{D1Database, Date, Env, console_error, console_log};

use crate::AppState;
use crate::assets;
use crate::push::{self, Outcome, Subscription};

/// The browser's report on notification support and permission.
/// `granted` means the visitor allowed it, not that the server saved a
/// subscription: that is `saved`, which only the server sets.
#[derive(Signals, Serialize, Deserialize)]
pub struct NotificationSettings {
    permission: String,
    saved: bool,
    probe: String,
    problem: String,
}

/// What `<push-subscribe>` reports from the browser. It describes data from
/// outside the server, so the action validates it again.
#[derive(Signals, Serialize, Deserialize)]
pub struct SubscriptionInput {
    endpoint: String,
    p256dh: String,
    auth: String,
}

const PUSH_STATUS: CustomEvent<String> = CustomEvent::new("push-status");
const PUSH_SUBSCRIPTION: CustomEvent<SubscriptionInput> = CustomEvent::new("push-subscription");

pub const REGISTER: Action<NotificationSettings, SubscriptionInput> =
    Action::post("/notifications/subscription");
pub const SCHEDULE_PROBE: Action<NotificationSettings, ()> = Action::post("/notifications/probe");

/// How long after its time a probe is still sent: the 15-minute deadline of
/// the MVP.
const DEADLINE_MS: u64 = 15 * 60 * 1000;
const PROBE_DELAY_MS: u64 = 60 * 1000;

/// The hosts of the push services of Chrome, Firefox, Safari, and Edge. An
/// endpoint anywhere else is refused, so a forged request cannot make the
/// worker post to a host of its choosing.
const PUSH_HOST_SUFFIXES: &[&str] = &[
    "fcm.googleapis.com",
    "updates.push.services.mozilla.com",
    "push.apple.com",
    "notify.windows.com",
];

#[derive(Debug, PartialEq, Eq)]
pub enum InvalidSubscription {
    Endpoint,
    Keys,
}

impl IntoResponse for InvalidSubscription {
    fn into_response(self) -> Response {
        let message = match self {
            Self::Endpoint => "通知の宛先が不正です。",
            Self::Keys => "通知の鍵が不正です。",
        };
        (StatusCode::BAD_REQUEST, message).into_response()
    }
}

fn endpoint_is_allowed(endpoint: &str) -> bool {
    let Some(rest) = endpoint.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    endpoint.len() <= 2048
        && endpoint
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'\\')
        && !host.is_empty()
        && host.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
        && PUSH_HOST_SUFFIXES
            .iter()
            .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
}

/// The subscription in `input`, if its endpoint and keys can be used.
pub fn validate(input: &SubscriptionInput) -> Result<Subscription, InvalidSubscription> {
    if !endpoint_is_allowed(&input.endpoint) {
        return Err(InvalidSubscription::Endpoint);
    }
    let subscription = Subscription {
        endpoint: input.endpoint.clone(),
        p256dh: input.p256dh.clone(),
        auth: input.auth.clone(),
    };
    push::check_keys(&subscription).map_err(|_| InvalidSubscription::Keys)?;
    Ok(subscription)
}

/// A D1 or configuration failure. The visitor reads a fixed message and the
/// log keeps the cause.
pub struct StoreError(worker::Error);

impl From<worker::Error> for StoreError {
    fn from(error: worker::Error) -> Self {
        Self(error)
    }
}

impl IntoResponse for StoreError {
    fn into_response(self) -> Response {
        console_error!("notifications: {}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "保存できませんでした。もう一度お試しください。",
        )
            .into_response()
    }
}

type Db = SendWrapper<Rc<D1Database>>;

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS push_probe (\
    id INTEGER PRIMARY KEY CHECK (id = 1), \
    endpoint TEXT NOT NULL, p256dh TEXT NOT NULL, auth TEXT NOT NULL, \
    probe_at INTEGER)";

#[derive(Deserialize)]
struct Row {
    endpoint: String,
    p256dh: String,
    auth: String,
    probe_at: Option<f64>,
}

fn now_ms() -> u64 {
    Date::now().as_millis()
}

async fn save_subscription(db: Db, subscription: Subscription) -> Result<(), StoreError> {
    SendFuture::new(async move {
        db.exec(SCHEMA).await?;
        db.prepare(
            "INSERT INTO push_probe (id, endpoint, p256dh, auth) VALUES (1, ?1, ?2, ?3) \
             ON CONFLICT (id) DO UPDATE SET endpoint = excluded.endpoint, \
             p256dh = excluded.p256dh, auth = excluded.auth, probe_at = NULL",
        )
        .bind(&[
            subscription.endpoint.into(),
            subscription.p256dh.into(),
            subscription.auth.into(),
        ])?
        .run()
        .await?;
        Ok(())
    })
    .await
}

/// Sets the probe time. Returns whether there was a subscription to set it on.
async fn schedule_probe(db: Db, at_ms: u64) -> Result<bool, StoreError> {
    SendFuture::new(async move {
        db.exec(SCHEMA).await?;
        #[expect(
            clippy::cast_precision_loss,
            reason = "millisecond timestamps fit in f64"
        )]
        let result = db
            .prepare("UPDATE push_probe SET probe_at = ?1 WHERE id = 1")
            .bind(&[(at_ms as f64).into()])?
            .run()
            .await?;
        Ok(result.meta()?.and_then(|meta| meta.changes).unwrap_or(0) > 0)
    })
    .await
}

pub async fn settings_page(state: AppState, (): ()) -> Page<NotificationSettings> {
    settings_view(&state.vapid_public)
}

fn settings_view(vapid_public: &str) -> Page<NotificationSettings> {
    let permission = NotificationSettings::permission();
    let saved = NotificationSettings::saved();
    let probe = NotificationSettings::probe();
    let problem = NotificationSettings::problem();
    let vapid_missing = vapid_public.is_empty();
    let initial = NotificationSettings {
        permission: "unknown".to_owned(),
        saved: false,
        probe: String::new(),
        problem: if vapid_missing {
            "VAPID公開鍵が設定されていません。".to_owned()
        } else {
            String::new()
        },
    };
    Page::new("通知設定", initial)
        .global(hypersig_ui::global())
        .theme(hypersig_ui::theme())
        .icon(assets::ICON_192)
        .script(assets::SUBSCRIBE_SCRIPT)
        .child(
            main()
                .class(Class::new("shell"))
                .child(h1().text("通知設定"))
                .child(
                    card()
                        .child(
                            card_header()
                                .child(card_title(Level::H2).text("プッシュ通知"))
                                .child(
                                    card_description().text("許可と購読の保存は別々に確認します。"),
                                ),
                        )
                        .child(
                            card_content()
                                .child(
                                    el("push-subscribe")
                                        .attr("vapid-key", vapid_public)
                                        .attr("worker", assets::SERVICE_WORKER)
                                        .attr("scope", assets::SCOPE)
                                        .attr("label", "通知を有効にする")
                                        .on_custom(PUSH_STATUS, move |status| {
                                            permission.set(status)
                                        })
                                        .on_custom(PUSH_SUBSCRIPTION, move |detail| {
                                            REGISTER.call_with(detail).error(problem)
                                        }),
                                )
                                .child(
                                    p().text("ブラウザの許可: ")
                                        .child(el("strong").bind_text(permission)),
                                )
                                .child(
                                    div()
                                        .child(
                                            p().show(saved).text("購読をサーバーに保存しました。"),
                                        )
                                        .child(p().show(!saved).text("購読は未保存です。")),
                                )
                                .child(
                                    button(ButtonVariant::Outline, ButtonSize::Lg)
                                        .on_click(SCHEDULE_PROBE.call().error(problem))
                                        .text("1分後にテスト通知を送る"),
                                )
                                .child(p().bind_text(probe))
                                .child(
                                    alert(AlertVariant::Destructive)
                                        .show(!problem.is_empty())
                                        .child(alert_title().text("通知を設定できません"))
                                        .child(alert_description().bind_text(problem)),
                                ),
                        ),
                ),
        )
}

pub async fn register(
    state: AppState,
    input: SubscriptionInput,
) -> Result<Patches<NotificationSettings>, ProbeError> {
    let subscription = validate(&input)?;
    save_subscription(state.db.clone(), subscription).await?;
    Ok(NotificationSettings::saved()
        .patch(true)
        .and(NotificationSettings::problem().patch(String::new()))
        .into())
}

pub async fn schedule(
    state: AppState,
    (): (),
) -> Result<Patches<NotificationSettings>, ProbeError> {
    let at = now_ms() + PROBE_DELAY_MS;
    if !schedule_probe(state.db.clone(), at).await? {
        return Err(ProbeError::NotSubscribed);
    }
    Ok(NotificationSettings::probe()
        .patch("約1分後に毎分の定期実行がテスト通知を送ります。".to_owned())
        .into())
}

pub enum ProbeError {
    Invalid(InvalidSubscription),
    Store(StoreError),
    NotSubscribed,
}

impl From<InvalidSubscription> for ProbeError {
    fn from(error: InvalidSubscription) -> Self {
        Self::Invalid(error)
    }
}

impl From<StoreError> for ProbeError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl IntoResponse for ProbeError {
    fn into_response(self) -> Response {
        match self {
            Self::Invalid(error) => error.into_response(),
            Self::Store(error) => error.into_response(),
            Self::NotSubscribed => (
                StatusCode::CONFLICT,
                "先に通知を有効にして購読を保存してください。",
            )
                .into_response(),
        }
    }
}

/// The scheduled invocation: sends the probe if its time has come and it is
/// within the deadline. The claim is a conditional update, so two
/// invocations do not both send it.
pub async fn run_scheduled(env: &Env) -> worker::Result<()> {
    let db = env.d1("DB")?;
    let private_key = env.secret("VAPID_PRIVATE_KEY")?.to_string();
    let subject = env.secret("VAPID_SUBJECT")?.to_string();
    let now = now_ms();
    db.exec(SCHEMA).await?;
    #[expect(
        clippy::cast_precision_loss,
        reason = "millisecond timestamps fit in f64"
    )]
    let (now_f, oldest) = (now as f64, now.saturating_sub(DEADLINE_MS) as f64);
    let Some(row) = db
        .prepare("SELECT endpoint, p256dh, auth, probe_at FROM push_probe WHERE id = 1 AND probe_at IS NOT NULL AND probe_at <= ?1 AND probe_at >= ?2")
        .bind(&[now_f.into(), oldest.into()])?
        .first::<Row>(None)
        .await?
    else {
        return Ok(());
    };
    let Some(probe_at) = row.probe_at else {
        return Ok(());
    };
    let claimed = db
        .prepare("UPDATE push_probe SET probe_at = NULL WHERE id = 1 AND probe_at = ?1")
        .bind(&[probe_at.into()])?
        .run()
        .await?;
    if claimed.meta()?.and_then(|meta| meta.changes).unwrap_or(0) == 0 {
        return Ok(());
    }
    let subscription = Subscription {
        endpoint: row.endpoint,
        p256dh: row.p256dh,
        auth: row.auth,
    };
    let sent = push::send(&subscription, b"probe", &subject, &private_key, now / 1000).await;
    let outcome = match sent {
        Ok(outcome) => outcome,
        Err(error) => {
            restore_claim(&db, &subscription.endpoint, probe_at).await?;
            return Err(error);
        }
    };
    console_log!("push probe: {outcome:?}");
    match outcome {
        Outcome::Expired => {
            db.prepare("DELETE FROM push_probe WHERE id = 1 AND endpoint = ?1")
                .bind(&[subscription.endpoint.as_str().into()])?
                .run()
                .await?;
        }
        Outcome::Retry => {
            restore_claim(&db, &subscription.endpoint, probe_at).await?;
        }
        Outcome::Accepted | Outcome::Rejected(_) => {}
    }
    Ok(())
}

/// Puts the claimed probe time back, unless the subscription was replaced or
/// a newer probe was reserved while the send was in flight.
async fn restore_claim(db: &D1Database, endpoint: &str, probe_at: f64) -> worker::Result<()> {
    db.prepare(
        "UPDATE push_probe SET probe_at = ?1 WHERE id = 1 AND probe_at IS NULL AND endpoint = ?2",
    )
    .bind(&[probe_at.into(), endpoint.into()])?
    .run()
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(endpoint: &str) -> SubscriptionInput {
        SubscriptionInput {
            endpoint: endpoint.to_owned(),
            p256dh: "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4".to_owned(),
            auth: "BTBZMqHH6r4Tts7J_aSIgg".to_owned(),
        }
    }

    #[test]
    fn refuses_endpoints_the_fetch_parser_reads_as_another_host() {
        for endpoint in [
            "https://example.org\\x.fcm.googleapis.com/probe",
            "https://example.org#.fcm.googleapis.com/probe",
            "https://example.org?.fcm.googleapis.com/probe",
            "https://FCM.googleapis.com/x",
            "https://fcm.googleapis.com/a b",
        ] {
            assert!(validate(&input(endpoint)).is_err(), "{endpoint}");
        }
    }

    #[test]
    fn accepts_known_push_services() {
        for endpoint in [
            "https://fcm.googleapis.com/fcm/send/abc",
            "https://web.push.apple.com/QGx",
            "https://updates.push.services.mozilla.com/wpush/v2/abc",
            "https://jp1.notify.windows.com/?token=abc",
        ] {
            assert!(validate(&input(endpoint)).is_ok(), "{endpoint}");
        }
    }

    #[test]
    fn refuses_other_hosts_and_schemes() {
        for endpoint in [
            "http://fcm.googleapis.com/x",
            "https://evil.example/x",
            "https://fcm.googleapis.com.evil.example/x",
            "https://fcm.googleapis.com@evil.example/x",
            "https://evilfcm.googleapis.com.example/x",
            "",
        ] {
            assert_eq!(
                validate(&input(endpoint)),
                Err(InvalidSubscription::Endpoint),
                "{endpoint}"
            );
        }
    }

    #[test]
    fn refuses_bad_keys() {
        let mut bad = input("https://fcm.googleapis.com/x");
        bad.p256dh = String::new();
        assert_eq!(validate(&bad), Err(InvalidSubscription::Keys));
        let mut bad = input("https://fcm.googleapis.com/x");
        bad.auth = "short".to_owned();
        assert_eq!(validate(&bad), Err(InvalidSubscription::Keys));
    }

    #[test]
    fn page_renders_the_typed_wiring_and_hides_nothing_when_unsaved() {
        let html = settings_view("BKey").render();
        assert!(html.contains("push-subscribe"));
        assert!(html.contains(assets::SUBSCRIBE_SCRIPT));
        assert!(html.contains("data-hs-on-custom"));
        assert!(html.contains("/notifications/subscription"));
    }
}
