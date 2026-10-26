//! The notification settings page and the verification of Web Push on a
//! worker: a browser subscribes, the server saves and checks the
//! subscription, and a scheduled invocation sends the probe notification.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use hypersig::html::{Class, CustomEvent, El, div, el, h1, h2, main, p, section, span};
use hypersig::{Action, Page, Patches, Sig, Signals, if_else};
use serde::{Deserialize, Serialize};
use worker::{Date, Env, console_error};

use crate::AppState;
use crate::assets;
use crate::medication_style::{STYLE, Tab, head, tab_bar, theme};
use crate::push::{self, Subscription};

/// The browser's report on notification support and permission.
/// `granted` means the visitor allowed it, not that the server saved a
/// subscription: that is `saved`, which only the server sets.
#[derive(Signals, Serialize, Deserialize)]
pub struct NotificationSettings {
    support: String,
    permission: String,
    browser_error: bool,
    standalone: bool,
    saved: bool,
    subscription_state: String,
    probe: String,
    problem: String,
    status: i32,
    removed: String,
}

/// What `<push-subscribe>` reports from the browser. It describes data from
/// outside the server, so the action validates it again.
#[derive(Signals, Serialize, Deserialize)]
pub struct SubscriptionInput {
    endpoint: String,
    p256dh: String,
    auth: String,
}

#[derive(Signals, Serialize, Deserialize)]
struct BrowserStatus {
    supported: bool,
    permission: String,
    failed: bool,
    standalone: bool,
}

const PUSH_STATUS: CustomEvent<BrowserStatus> = CustomEvent::new("push-status");
const PUSH_SUBSCRIPTION: CustomEvent<SubscriptionInput> = CustomEvent::new("push-subscription");
const PUSH_CHECK: CustomEvent<String> = CustomEvent::new("push-check");
const PUSH_REMOVE: CustomEvent<String> = CustomEvent::new("push-remove");
const PUSH_TEST: CustomEvent<String> = CustomEvent::new("push-test");
const PUSH_MISSING: CustomEvent<()> = CustomEvent::new("push-missing");

pub const REGISTER: Action<NotificationSettings, SubscriptionInput> =
    Action::post("/notifications/subscription");
pub const SCHEDULE_PROBE: Action<NotificationSettings, String> =
    Action::post("/notifications/probe");
pub const UNREGISTER: Action<NotificationSettings, String> =
    Action::post("/notifications/unsubscribe");
pub const SUBSCRIPTION_STATUS: Action<NotificationSettings, String> =
    Action::post("/notifications/subscription/status");

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

/// A D1 failure. Neither the public message nor the log includes stored values.
pub struct StoreError;

impl From<worker::Error> for StoreError {
    fn from(_: worker::Error) -> Self {
        Self
    }
}

impl IntoResponse for StoreError {
    fn into_response(self) -> Response {
        console_error!("notifications: storage failure");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "保存できませんでした。もう一度お試しください。",
        )
            .into_response()
    }
}

fn now_ms() -> u64 {
    Date::now().as_millis()
}

pub async fn settings_page(
    state: AppState,
    _: crate::auth::Member,
    (): (),
) -> Page<NotificationSettings> {
    settings_view(&state.vapid_public)
}

fn settings_view(vapid_public: &str) -> Page<NotificationSettings> {
    let permission = NotificationSettings::permission();
    let saved = NotificationSettings::saved();
    let probe = NotificationSettings::probe();
    let problem = NotificationSettings::problem();
    let vapid_missing = vapid_public.is_empty();
    let initial = NotificationSettings {
        support: "unknown".to_owned(),
        permission: "unknown".to_owned(),
        browser_error: false,
        standalone: false,
        saved: false,
        subscription_state: "unknown".to_owned(),
        status: if vapid_missing {
            i32::from(StatusCode::SERVICE_UNAVAILABLE.as_u16())
        } else {
            0
        },
        removed: String::new(),
        probe: String::new(),
        problem: if vapid_missing {
            "VAPID公開鍵が設定されていません。".to_owned()
        } else {
            String::new()
        },
    };
    let granted = permission.equals("granted");
    let ready = granted
        .clone()
        .and(saved)
        .and(NotificationSettings::support().equals("supported"))
        .and(!NotificationSettings::browser_error());
    Page::new("通知設定", initial)
        .global(hypersig_ui::global())
        .theme(hypersig_ui::theme())
        .theme(theme())
        .style(STYLE)
        .icon(assets::ICON_192)
        .script(assets::SUBSCRIBE_SCRIPT)
        .child(
            main()
                .class(Class::new("med-shell"))
                .child(head(
                    h1().text("通知設定"),
                    p().text("いつもの端末に、そっとお知らせ。"),
                ))
                .child(crate::failure::alerts(problem, NotificationSettings::status()))
                .child(
                    section()
                        .class(Class::new("med-panel"))
                        .attr("aria-labelledby", "ready-heading")
                        .child(
                            h2().id("ready-heading")
                                .class(Class::new("med-panel-title"))
                                .bind_text(if_else(
                                    ready,
                                    "✓ 通知の準備ができています",
                                    "通知の準備ができていません",
                                )),
                        )
                        .child(browser_status_rows())
                        .child(
                            div()
                                .class(Class::new("med-row"))
                                .child(span().text("この端末の通知先"))
                                .child(el("strong").bind_text(if_else(
                                    saved,
                                    "保存済み",
                                    "未保存",
                                ))),
                        )
                        .child(subscription_control(vapid_public)),
                )
                .child(subscription_guidance())
                .child(preview_section())
                .child(p().show(NotificationSettings::browser_error()).text("端末の通知操作に失敗しました。通信とブラウザの設定を確認し、再設定または解除をやり直してください。"))
                .child(p().text("iPhone／iPadではSafariからホーム画面に追加して開いてください。拒否した場合は端末の通知設定を変更し、再設定してください。許可済みでも未保存・失効の場合は通知を有効にしてください。"))
                .child(probe_section(probe))
                .child(
                    div()
                        .class(Class::new("med-actions"))
                        .child(crate::auth::sign_out_button()),
                )
                .child(tab_bar(Tab::Settings)),
        )
}

fn browser_status_rows() -> El<NotificationSettings> {
    let permission = NotificationSettings::permission();
    let support = NotificationSettings::support();
    div()
        .child(
            div()
                .class(Class::new("med-row"))
                .child(span().text("利用形態"))
                .child(el("strong").bind_text(if_else(
                    NotificationSettings::standalone(),
                    "ホーム画面アプリ",
                    "ブラウザ",
                ))),
        )
        .child(
            div()
                .class(Class::new("med-row"))
                .child(span().text("対応状況"))
                .child(el("strong").bind_text(if_else(
                    support.equals("unknown"),
                    "未確認",
                    if_else(support.equals("supported"), "対応", "非対応"),
                ))),
        )
        .child(
            div()
                .class(Class::new("med-row"))
                .child(span().text("ブラウザの許可"))
                .child(el("strong").bind_text(if_else(
                    permission.equals("granted"),
                    "許可済み",
                    if_else(
                        permission.equals("denied"),
                        "拒否",
                        if_else(permission.equals("default"), "未許可", "未確認"),
                    ),
                ))),
        )
}

fn subscription_guidance() -> El<NotificationSettings> {
    let state = NotificationSettings::subscription_state();
    div()
        .child(p().show(state.equals("missing")).text("この端末の購読は未登録です。"))
        .child(p().show(state.equals("inactive")).text("この購読は失効または解除済みです。通知を有効にして再登録してください。"))
        .child(p().show(!NotificationSettings::standalone()).text("ブラウザから利用中です。iPhone／iPadでは、ホーム画面に追加したアイコンから開いてください。"))
}

fn subscription_control(vapid_public: &str) -> El<NotificationSettings> {
    let problem = NotificationSettings::problem();
    let status = NotificationSettings::status();
    el("push-subscribe")
        .attr("vapid-key", vapid_public)
        .attr("worker", assets::SERVICE_WORKER)
        .attr("scope", assets::SCOPE)
        .attr("label", "通知を有効にする")
        .bind_attr("removed-endpoint", NotificationSettings::removed())
        .on_custom(PUSH_CHECK, move |endpoint| {
            NotificationSettings::saved().set(false).then(
                SUBSCRIPTION_STATUS
                    .call_with(endpoint)
                    .error(problem)
                    .status(status),
            )
        })
        .on_custom(PUSH_REMOVE, move |endpoint| {
            NotificationSettings::saved()
                .set(false)
                .then(UNREGISTER.call_with(endpoint).error(problem).status(status))
        })
        .on_custom(PUSH_TEST, move |endpoint| {
            SCHEDULE_PROBE
                .call_with(endpoint)
                .error(problem)
                .status(status)
        })
        .on_custom(PUSH_MISSING, |_| {
            NotificationSettings::saved()
                .set(false)
                .then(NotificationSettings::subscription_state().set("missing"))
        })
        .on_custom(PUSH_STATUS, move |detail| {
            NotificationSettings::support()
                .set(if_else(
                    detail.clone().at(BrowserStatus::supported()),
                    "supported",
                    "unsupported",
                ))
                .then(
                    NotificationSettings::permission()
                        .set(detail.clone().at(BrowserStatus::permission())),
                )
                .then(
                    NotificationSettings::browser_error()
                        .set(detail.clone().at(BrowserStatus::failed())),
                )
                .then(
                    NotificationSettings::standalone().set(detail.at(BrowserStatus::standalone())),
                )
        })
        .on_custom(PUSH_SUBSCRIPTION, move |detail| {
            NotificationSettings::saved()
                .set(false)
                .then(REGISTER.call_with(detail).error(problem).status(status))
        })
}

fn preview_section() -> El<NotificationSettings> {
    section()
        .class(Class::new("med-section"))
        .attr("aria-labelledby", "preview-heading")
        .child(
            h2().id("preview-heading")
                .class(Class::new("med-section-title"))
                .text("通知のプレビュー"),
        )
        .child(
            div()
                .class(Class::new("med-preview"))
                .child(
                    span()
                        .class(Class::new("med-icon"))
                        .attr("aria-hidden", "true")
                        .text("葉"),
                )
                .child(
                    span()
                        .class(Class::new("med-preview-text"))
                        .child(el("strong").text("服薬の時間です"))
                        .child(span().text("アプリで今日の予定を確認してください")),
                ),
        )
        .child(
            p().class(Class::new("med-muted"))
                .text("薬名や量は通知に表示しません。"),
        )
}

fn probe_section(probe: Sig<NotificationSettings, String>) -> El<NotificationSettings> {
    section()
        .class(Class::new("med-section"))
        .attr("aria-labelledby", "probe-heading")
        .child(
            h2().id("probe-heading")
                .class(Class::new("med-section-title"))
                .text("この端末で受信を確かめる"),
        )
        .child(
            p().class(Class::new("med-muted"))
                .text("1分後に、テスト通知を予約します。"),
        )
        .child(
            p().class(Class::new("med-muted"))
                .show(!probe.is_empty())
                .bind_text(probe),
        )
        .child(
            p().class(Class::new("med-note"))
                .text("時刻は Asia/Tokyo（日本時間）で固定です。"),
        )
}

pub async fn register(
    state: AppState,
    member: crate::auth::Member,
    input: SubscriptionInput,
) -> Result<Patches<NotificationSettings>, ProbeError> {
    let subscription = validate(&input)?;
    crate::repository::subscribe(
        &state.db,
        member.owner(),
        subscription,
        hypersig::Timestamp::now().as_millis(),
    )
    .await
    .map_err(ProbeError::Repository)?;
    Ok(NotificationSettings::saved()
        .patch(true)
        .and(NotificationSettings::subscription_state().patch("active".to_owned()))
        .and(NotificationSettings::removed().patch(String::new()))
        .and(NotificationSettings::problem().patch(String::new()))
        .into())
}

pub async fn schedule(
    state: AppState,
    member: crate::auth::Member,
    endpoint: String,
) -> Result<Patches<NotificationSettings>, ProbeError> {
    if !endpoint_is_allowed(&endpoint) {
        return Err(InvalidSubscription::Endpoint.into());
    }
    let at = now_ms() + PROBE_DELAY_MS;
    if !crate::repository::schedule_probe(&state.db, member.owner(), &endpoint, at)
        .await
        .map_err(StoreError::from)?
    {
        return Err(ProbeError::NotSubscribed);
    }
    Ok(NotificationSettings::probe()
        .patch(
            "テスト通知の予約を受け付けました。端末への表示は端末で確認してください。".to_owned(),
        )
        .into())
}

pub enum ProbeError {
    Repository(crate::medications::Error),
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
            Self::Repository(error) => error.into_response(),
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

pub async fn unregister(
    state: AppState,
    member: crate::auth::Member,
    endpoint: String,
) -> Result<Patches<NotificationSettings>, ProbeError> {
    crate::repository::unsubscribe(
        &state.db,
        member.owner(),
        &endpoint,
        hypersig::Timestamp::now().as_millis(),
    )
    .await
    .map_err(ProbeError::Repository)?;
    Ok(NotificationSettings::saved()
        .patch(false)
        .and(NotificationSettings::subscription_state().patch("inactive".to_owned()))
        .and(NotificationSettings::removed().patch(endpoint))
        .into())
}

pub async fn subscription_status(
    state: AppState,
    member: crate::auth::Member,
    endpoint: String,
) -> Result<Patches<NotificationSettings>, ProbeError> {
    if !endpoint_is_allowed(&endpoint) {
        return Err(InvalidSubscription::Endpoint.into());
    }
    let state = crate::repository::subscription_state(&state.db, member.owner(), &endpoint)
        .await
        .map_err(ProbeError::Repository)?;
    let (saved, status) = match state {
        crate::repository::SubscriptionState::Enabled => (true, "active"),
        crate::repository::SubscriptionState::Disabled => (false, "inactive"),
        crate::repository::SubscriptionState::Missing => (false, "missing"),
    };
    Ok(NotificationSettings::saved()
        .patch(saved)
        .and(NotificationSettings::subscription_state().patch(status.to_owned()))
        .into())
}

/// Both queues use durable claims and the same sender and deadline policy.
pub async fn run_scheduled(env: &Env) -> worker::Result<()> {
    let scheduled = crate::delivery::run(env).await;
    let probe = crate::probe_delivery::run(env).await;
    scheduled.and(probe)
}

/// An object, rather than a formatted string, makes the fields queryable in
/// Workers Logs. Never include the subscription, owner, keys or payload.
pub(crate) fn log_push_result(outcome: &str, status: Option<u16>) {
    let json = serde_json::json!({
        "event": "push_probe_result",
        "outcome": outcome,
        "status": status,
    });
    let value = js_sys::JSON::parse(&json.to_string()).expect("serde_json produces valid JSON");
    worker::web_sys::console::log_1(&value);
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
        assert!(!html.contains("利用者 ID"));
        assert!(html.contains(crate::MEDICATIONS.path()));
        assert!(html.contains(r#"aria-current="page""#));
    }
}
