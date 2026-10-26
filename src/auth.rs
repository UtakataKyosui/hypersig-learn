//! Sign-in with Google, and the `Member` every protected handler requires.
//!
//! The browser holds one cookie with a session id. The session record, the
//! user and the OAuth flows live in D1 (`auth_store`, `session_store`).
use crate::{AppState, assets, auth_store, oidc, session_state};
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use chrono::DateTime;
use hypersig::Route;
use hypersig::{Action, Page, PageResponse, Patches, RequestKind, Signals};
use serde::{Deserialize, Serialize};
use std::future::Future;
use time::OffsetDateTime;
use tower_sessions::Session;

/// The login flow ends this long after it starts.
const FLOW_LIFETIME_MS: i64 = 10 * 60 * 1000;
const BINDING: &str = "login_binding";
const NOTICE: &str = "login_notice";

/// Google redirects the visitor here. It is a plain route: the request is a
/// cross-site navigation, which an action would refuse, and the flow's
/// `state` bound to the browser takes the place of a CSRF check.
pub const CALLBACK: &str = "/auth/google/callback";

/// The signed-in user. `owner` is the application's own user id, taken from
/// the server-side session and never from a request.
#[derive(Clone, Debug)]
pub struct Member(String);
impl Member {
    pub fn owner(&self) -> &str {
        &self.0
    }
}
impl<S: Sync> FromRequestParts<S> for Member {
    type Rejection = StatusCode;
    fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        std::future::ready(
            parts
                .extensions
                .get::<Self>()
                .cloned()
                .ok_or(StatusCode::UNAUTHORIZED),
        )
    }
}

/// The signed-in member of `session`, if the user is still active and the
/// session was not revoked since it was issued.
async fn member_of<L, F>(session: &Session, lookup: L) -> Option<Member>
where
    L: FnOnce(String) -> F,
    F: Future<Output = Option<auth_store::User>>,
{
    let identity = session_state::current(session).await?;
    let user = lookup(identity.user_id.clone()).await?;
    session_state::honoured(&identity, &user, auth_store::now_ms() / 1000)
        .then_some(Member(user.id))
}

fn public(path: &str) -> bool {
    [
        assets::MANIFEST,
        assets::SERVICE_WORKER,
        assets::SUBSCRIBE_SCRIPT,
        assets::ICON_192,
        assets::ICON_512,
        hypersig::RUNTIME_PATH,
        LOGIN.path(),
        START.path(),
        CALLBACK,
    ]
    .contains(&path)
}

/// Whether `path` is one of the application's pages: the fixed routes and
/// everything under the medications page, which has a page per medication.
fn is_page(path: &str) -> bool {
    [
        crate::INDEX.path(),
        crate::COUNTER.path(),
        crate::NOTIFICATIONS.path(),
    ]
    .contains(&path)
        || path == crate::MEDICATIONS.path()
        || path
            .strip_prefix(crate::MEDICATIONS.path())
            .is_some_and(|rest| rest.starts_with('/'))
}

/// A page navigation goes to the login page; everything else, including an
/// action or stream the runtime fetches, is refused with 401.
fn refusal(parts: &Parts) -> Response {
    if parts.method == axum::http::Method::GET
        && RequestKind::of(parts) == RequestKind::Navigation
        && is_page(parts.uri.path())
    {
        Redirect::to(LOGIN.url(()).as_str()).into_response()
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

async fn guard<L, F>(session: Session, mut request: Request, next: Next, lookup: L) -> Response
where
    L: FnOnce(String) -> F,
    F: Future<Output = Option<auth_store::User>>,
{
    if public(request.uri().path()) {
        return next.run(request).await;
    }
    if let Some(member) = member_of(&session, lookup).await {
        request.extensions_mut().insert(member);
        let mut response = next.run(request).await;
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        return response;
    }
    let (parts, _) = request.into_parts();
    refusal(&parts)
}

pub async fn protect(
    State(state): State<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Response {
    guard(session, request, next, |id| async move {
        auth_store::user(&state.db, &id).await.ok().flatten()
    })
    .await
}

#[derive(Signals, Serialize, Deserialize)]
pub struct Login {
    problem: String,
}
pub const SIGN_OUT: Action<Login, ()> = Action::post("/auth/logout");

/// The sign-out button for any page. The action is the one `SIGN_OUT` serves;
/// only the page's state type differs.
pub fn sign_out_button<S>() -> hypersig::html::El<S> {
    use hypersig_ui::{ButtonSize, ButtonVariant, button};
    button(ButtonVariant::Outline, ButtonSize::Default)
        .on_click(Action::<S>::post(SIGN_OUT.path()).call())
        .text("ログアウト")
}
hypersig::routes! {
    fn routes(Router<AppState>);
    pub(crate) const LOGIN: Route = "/login" => page;
    const START: Route = "/auth/google" => start;
}

/// Why a sign-in did not complete. The page shows a fixed message for each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Notice {
    Cancelled,
    Invalid,
    Failed,
    Unavailable,
}
impl Notice {
    fn code(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Invalid => "invalid",
            Self::Failed => "failed",
            Self::Unavailable => "unavailable",
        }
    }
    fn message(code: &str) -> &'static str {
        match code {
            "cancelled" => "Google でのログインが取り消されました。",
            "invalid" => "ログインの手続きが無効または期限切れです。もう一度お試しください。",
            "unavailable" => "このアカウントは利用できません。",
            _ => "ログインできませんでした。時間をおいてもう一度お試しください。",
        }
    }
}

async fn page(_: AppState, session: Session, (): ()) -> Page<Login> {
    use hypersig::html::{a, main};
    use hypersig_ui::{
        AlertVariant, ButtonSize, ButtonVariant, Level, alert, alert_description, alert_title,
        button_style, card, card_content, card_description, card_header, card_title,
    };
    let notice = session.get::<String>(NOTICE).await.ok().flatten();
    if notice.is_some() {
        pin(&session).await;
        let _ = session.remove::<String>(NOTICE).await;
    }
    let problem = notice
        .map(|code| Notice::message(&code).to_owned())
        .unwrap_or_default();
    Page::new(
        "ログイン",
        Login {
            problem: problem.clone(),
        },
    )
    .global(hypersig_ui::global())
    .theme(hypersig_ui::theme())
    .child(
        main()
            .attr("style", "max-width:28rem;margin:0 auto;padding:4rem 1rem")
            .child(
                card()
                    .child(
                        card_header()
                            .child(card_title(Level::H2).text("ログイン"))
                            .child(card_description().text("Google アカウントでログインします。")),
                    )
                    .child(
                        card_content()
                            .child(
                                a().href(START.link())
                                    .css(button_style(ButtonVariant::Default, ButtonSize::Lg))
                                    .attr("style", "text-decoration:none")
                                    .text("Google でログイン"),
                            )
                            .child(
                                alert(AlertVariant::Destructive)
                                    .show(!Login::problem().is_empty())
                                    .child(alert_title().text("ログインできません"))
                                    .child(alert_description().bind_text(Login::problem())),
                            ),
                    ),
            ),
    )
}

/// What Google appended to the callback URL.
#[derive(Debug, Default, PartialEq, Eq)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    denied: bool,
}
fn parse_callback(query: Option<&str>) -> CallbackQuery {
    let mut parsed = CallbackQuery::default();
    for (name, value) in url::form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        match name.as_ref() {
            "code" if parsed.code.is_none() && value.len() <= 2048 => {
                parsed.code = Some(value.into_owned());
            }
            "state" if parsed.state.is_none() && value.len() <= 256 => {
                parsed.state = Some(value.into_owned());
            }
            "error" => parsed.denied = true,
            _ => {}
        }
    }
    parsed
}

fn log_failure(kind: &str) {
    #[cfg(target_arch = "wasm32")]
    worker::console_error!("auth: {kind}");
    #[cfg(not(target_arch = "wasm32"))]
    let _ = kind;
}

/// Keeps a signed-in session's deadline when a sign-in route changes it.
async fn pin(session: &Session) {
    if let Some(identity) = session_state::current(session).await {
        session_state::pin_expiry(session, &identity);
    }
}

/// Sends the visitor back to the login page with a fixed notice.
async fn rejected(session: &Session, notice: Notice) -> Response {
    pin(session).await;
    // Only a browser that started a flow or is signed in has a session row to
    // carry the notice. Any other request must not create one.
    let known = session
        .get::<String>(BINDING)
        .await
        .ok()
        .flatten()
        .is_some()
        || session_state::current(session).await.is_some();
    if known {
        let _ = session.insert(NOTICE, notice.code()).await;
    }
    private(Redirect::to(LOGIN.url(()).as_str()).into_response())
}

/// The sign-in URLs carry a code or lead to one: no caching, no referrer.
fn private(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

/// Redirects to Google. A page route may answer with any response through its
/// `Err`, which is how the redirect to another site is sent; the `Ok` redirect
/// returns the visitor to the login page when the flow could not start.
#[expect(
    clippy::result_large_err,
    reason = "a page handler answers with an arbitrary response through its `Err`"
)]
async fn start(
    state: AppState,
    (session, headers): (Session, HeaderMap),
    (): (),
) -> Result<PageResponse, Response> {
    pin(&session).await;
    let now = auth_store::now_ms();
    // Expired flows and sessions are removed as visitors arrive, and by cron.
    let _ = auth_store::purge_expired(&state.db, now).await;
    let slot = async {
        let reservation = auth_store::random_token().ok()?;
        let binding = auth_store::random_token().ok()?;
        let reserved = auth_store::reserve_flow(
            &state.db,
            &reservation,
            &binding,
            &client_key(&headers),
            now,
            now + FLOW_LIFETIME_MS,
            MAX_LIVE_FLOWS,
        )
        .await
        .ok()?;
        Some(reserved.map(|()| (reservation, binding)))
    }
    .await;
    let (reservation, binding) = match slot {
        Some(Some(slot)) => slot,
        Some(None) => return Err(throttled()),
        None => {
            log_failure("start");
            return Ok(PageResponse::redirect(LOGIN.url(())));
        }
    };
    let flow = async {
        let started = oidc::start(&oidc::Fetch, &state.oidc).await.ok()?;
        auth_store::fill_flow(
            &state.db,
            &reservation,
            &started.state,
            &started.nonce,
            &started.pkce_verifier,
        )
        .await
        .ok()?;
        session.insert(BINDING, binding).await.ok()?;
        Some(started.url)
    }
    .await;
    if let Some(url) = flow {
        return Err(private(Redirect::to(url.as_str()).into_response()));
    }
    log_failure("start");
    let _ = session.insert(NOTICE, Notice::Failed.code()).await;
    Ok(PageResponse::redirect(LOGIN.url(())))
}

/// A client that holds this many unconsumed flows cannot start another until
/// one expires. Each start writes a flow row and at most one session row.
const MAX_LIVE_FLOWS: u32 = 10;

/// The connecting address Cloudflare reports, hashed, with an IPv6 address
/// reduced to its /64 network so that one subscriber's addresses share a slot.
fn client_key(headers: &HeaderMap) -> String {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use sha2::{Digest as _, Sha256};
    let address = headers
        .get("cf-connecting-ip")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown");
    let network = match address.parse::<std::net::Ipv6Addr>() {
        Ok(ip) => {
            let [a, b, c, d, ..] = ip.segments();
            format!("{a:x}:{b:x}:{c:x}:{d:x}")
        }
        Err(_) => address.to_owned(),
    };
    URL_SAFE_NO_PAD.encode(Sha256::digest(network.as_bytes()))
}

/// The answer to a client at its limit: a fixed text and no session row.
fn throttled() -> Response {
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        "ログインの開始が多すぎます。しばらくしてからもう一度お試しください。",
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from_static("600"));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    private(response)
}

async fn callback(State(state): State<AppState>, session: Session, uri: Uri) -> Response {
    let query = parse_callback(uri.query());
    let binding = session.get::<String>(BINDING).await.ok().flatten();
    let (Some(binding), Some(flow_state)) = (binding, query.state.as_deref()) else {
        return rejected(&session, Notice::Invalid).await;
    };
    let now = auth_store::now_ms();
    // The flow is consumed first, so a replay, a foreign browser and a late
    // callback all end here, whether Google reported success or not.
    let flow = match auth_store::consume_flow(&state.db, flow_state, &binding, now).await {
        Ok(Some(flow)) => flow,
        Ok(None) => return rejected(&session, Notice::Invalid).await,
        Err(_) => {
            log_failure("flow");
            return rejected(&session, Notice::Failed).await;
        }
    };
    if query.denied {
        return rejected(&session, Notice::Cancelled).await;
    }
    let Some(code) = query.code else {
        return rejected(&session, Notice::Invalid).await;
    };
    let Some(instant) = DateTime::from_timestamp_millis(now) else {
        return rejected(&session, Notice::Failed).await;
    };
    let verified = match oidc::finish(
        &oidc::Fetch,
        &state.oidc,
        &code,
        &flow.pkce_verifier,
        &flow.nonce,
        instant,
    )
    .await
    {
        Ok(verified) => verified,
        Err(error) => {
            log_failure(&format!("{error:?}"));
            return rejected(&session, Notice::Failed).await;
        }
    };
    let user = async {
        let new_id = auth_store::random_token().ok()?;
        auth_store::login(&state.db, &verified.issuer, &verified.subject, &new_id, now)
            .await
            .ok()
    }
    .await;
    let Some(user) = user else {
        log_failure("user");
        return rejected(&session, Notice::Failed).await;
    };
    if !user.active() {
        return rejected(&session, Notice::Unavailable).await;
    }
    let Ok(now_utc) = OffsetDateTime::from_unix_timestamp(now / 1000) else {
        return rejected(&session, Notice::Failed).await;
    };
    let _ = session.remove::<String>(BINDING).await;
    if session_state::sign_in(&session, &user, now_utc)
        .await
        .is_err()
    {
        log_failure("session");
        return rejected(&session, Notice::Failed).await;
    }
    private(Redirect::to(crate::INDEX.url(()).as_str()).into_response())
}

async fn sign_out(
    _: AppState,
    (_, session): (Member, Session),
    (): (),
) -> Result<Patches<Login>, StatusCode> {
    session_state::sign_out(&session)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Patches::new().navigate(LOGIN.url(())))
}

pub fn router(router: axum::Router<AppState>) -> axum::Router<AppState> {
    use hypersig::RouterExt as _;
    routes(router)
        .route(CALLBACK, get(callback))
        .action(SIGN_OUT, sign_out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_store::testing::Memory;
    use hypersig::RouterExt as _;
    use std::sync::Arc;
    use tower::ServiceExt as _;
    use tower_sessions::{SessionManagerLayer, SessionStore as _};

    fn from(address: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("cf-connecting-ip", address.parse().unwrap());
        headers
    }

    #[test]
    fn a_client_is_a_hash_and_an_ipv6_network_shares_one_slot() {
        let a = client_key(&from("203.0.113.7"));
        assert_ne!(a, client_key(&from("203.0.113.8")));
        assert!(!a.contains("203"));
        assert_eq!(
            client_key(&from("2001:db8:1:2:aaaa::1")),
            client_key(&from("2001:db8:1:2:bbbb::2"))
        );
        assert_ne!(
            client_key(&from("2001:db8:1:2::1")),
            client_key(&from("2001:db8:1:3::1"))
        );
        assert_eq!(client_key(&HeaderMap::new()), client_key(&from("unknown")));
    }

    #[test]
    fn the_throttled_answer_is_fixed_and_uncached() {
        let response = throttled();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "600");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(response.headers().get(header::SET_COOKIE).is_none());
    }

    fn user(version: i64, status: &str) -> auth_store::User {
        auth_store::User {
            id: "user-a".into(),
            status: status.into(),
            session_version: version,
        }
    }

    /// An app whose protected routes need a `Member`, behind a session layer
    /// over `store`, with the user directory `users`.
    fn app(store: &Arc<Memory>, users: Vec<auth_store::User>) -> axum::Router {
        let users = Arc::new(users);
        axum::Router::new()
            .route(
                crate::MEDICATIONS.path(),
                get(|member: Member| async move { member.owner().to_owned() }),
            )
            .route(LOGIN.path(), get(|| async { "login" }))
            .action(
                SIGN_OUT,
                |(): (), (member, session): (Member, Session), (): ()| async move {
                    session_state::sign_out(&session).await.unwrap();
                    Patches::from(Login::problem().patch(member.owner().to_owned()))
                },
            )
            .layer(axum::middleware::from_fn(
                move |session: Session, request: Request, next: Next| {
                    let users = users.clone();
                    guard(session, request, next, move |id| async move {
                        users
                            .iter()
                            .find(|user| user.id == id)
                            .map(|user| auth_store::User {
                                id: user.id.clone(),
                                status: user.status.clone(),
                                session_version: user.session_version,
                            })
                    })
                },
            ))
            .layer(SessionManagerLayer::new(store.as_ref().clone()))
    }

    /// Signs `user` in on a fresh session in `store`; returns the cookie.
    fn sign_in(store: &Arc<Memory>, user: &auth_store::User) -> String {
        futures::executor::block_on(async {
            let session = Session::new(None, store.clone(), None);
            let now = OffsetDateTime::now_utc();
            session_state::sign_in(&session, user, now).await.unwrap();
            session.save().await.unwrap();
            format!("id={}", session.id().unwrap())
        })
    }

    fn get_with(uri: &str, cookie: Option<&str>, html: bool) -> Request {
        let mut request = Request::builder().uri(uri);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        if html {
            request = request.header(header::ACCEPT, "text/html");
        }
        request.body(axum::body::Body::empty()).unwrap()
    }
    fn send(app: &axum::Router, request: Request) -> Response {
        futures::executor::block_on(app.clone().oneshot(request)).unwrap()
    }

    #[test]
    fn member_comes_from_the_server_side_session_and_is_never_cached() {
        let store = Arc::new(Memory::default());
        let a = user(1, "active");
        let cookie = sign_in(&store, &a);
        let app = app(&store, vec![a]);
        let response = send(
            &app,
            get_with(crate::MEDICATIONS.path(), Some(&cookie), true),
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = futures::executor::block_on(axum::body::to_bytes(response.into_body(), 64));
        assert_eq!(body.unwrap(), "user-a");
    }

    #[test]
    fn pages_redirect_while_actions_and_streams_get_401() {
        let app = app(&Arc::new(Memory::default()), vec![]);
        let response = send(&app, get_with(crate::MEDICATIONS.path(), None, true));
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[header::LOCATION], LOGIN.path());
        let stream: hypersig::StreamAction<Login> = hypersig::StreamAction::new("/medications");
        assert_eq!(
            send(&app, stream.request(&())).status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            send(&app, SIGN_OUT.request(&())).status(),
            StatusCode::UNAUTHORIZED
        );
        // The login page itself stays reachable.
        assert_eq!(
            send(&app, get_with(LOGIN.path(), None, true)).status(),
            StatusCode::OK
        );
    }

    #[test]
    fn medication_pages_redirect_and_edit_actions_get_401_when_signed_out() {
        let app = app(&Arc::new(Memory::default()), vec![]);
        let target = crate::medication_edit::Target {
            id: "med".into(),
            revision: 1,
        };
        for page in [
            crate::MEDICATION.url("med".into()),
            crate::NEW_MEDICATION.url(()),
        ] {
            let response = send(&app, get_with(page.as_str(), None, true));
            assert_eq!(response.status(), StatusCode::SEE_OTHER, "{page:?}");
            assert_eq!(response.headers()[header::LOCATION], LOGIN.path());
        }
        let form = crate::medication_form::MedicationForm::blank("2026-10-05".into());
        let slot = crate::medication_form::blank_slot();
        let requests = [
            crate::medication_edit::SAVE_MEDICATION.request(&(target.clone(), form)),
            crate::medication_edit::SAVE_SLOT.request(&(target.clone(), String::new(), slot)),
            crate::medication_edit::REMOVE_SLOT.request(&(target.clone(), "slot".into())),
            crate::pause_edit::SAVE_PAUSE.request(&(
                target.clone(),
                String::new(),
                crate::pause_edit::blank_pause(),
            )),
            crate::pause_edit::REMOVE_PAUSE.request(&(target, "pause".into())),
        ];
        for request in requests {
            assert_eq!(send(&app, request).status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[test]
    fn unknown_forged_disabled_and_revoked_sessions_are_sent_to_login() {
        let store = Arc::new(Memory::default());
        let a = user(2, "active");
        let cookie = sign_in(&store, &a);
        let refused = |users: Vec<auth_store::User>, cookie: &str| {
            let app = app(&store, users);
            send(
                &app,
                get_with(crate::MEDICATIONS.path(), Some(cookie), false),
            )
            .status()
        };
        assert_eq!(refused(vec![a], &cookie), StatusCode::OK);
        assert_eq!(
            refused(vec![user(2, "disabled")], &cookie),
            StatusCode::SEE_OTHER
        );
        assert_eq!(
            refused(vec![user(3, "active")], &cookie),
            StatusCode::SEE_OTHER
        );
        assert_eq!(refused(vec![], &cookie), StatusCode::SEE_OTHER);
        assert_eq!(
            refused(vec![user(2, "active")], "id=AAAAAAAAAAAAAAAAAAAAAA"),
            StatusCode::SEE_OTHER
        );
        // The previous password cookie is not a session.
        assert_eq!(
            refused(
                vec![user(2, "active")],
                "__Host-medication=owner:99999999999.x"
            ),
            StatusCode::SEE_OTHER
        );
    }

    #[test]
    fn a_session_past_its_deadline_is_refused_even_if_the_store_still_holds_it() {
        let store = Arc::new(Memory::default());
        let a = user(1, "active");
        let cookie = futures::executor::block_on(async {
            let session = Session::new(None, store.clone(), None);
            let signed_in = OffsetDateTime::now_utc() - session_state::LIFETIME * 2;
            session_state::sign_in(&session, &a, signed_in)
                .await
                .unwrap();
            session.save().await.unwrap();
            format!("id={}", session.id().unwrap())
        });
        let app = app(&store, vec![a]);
        let response = send(
            &app,
            get_with(crate::MEDICATIONS.path(), Some(&cookie), true),
        );
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
    }

    #[test]
    fn signing_out_removes_the_record() {
        let store = Arc::new(Memory::default());
        let a = user(1, "active");
        let cookie = sign_in(&store, &a);
        let app = app(&store, vec![a]);
        let mut request = SIGN_OUT.request(&());
        request
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        let response = send(&app, request);
        assert_eq!(response.status(), StatusCode::OK);
        let id: tower_sessions::session::Id = cookie.trim_start_matches("id=").parse().unwrap();
        assert!(
            futures::executor::block_on(store.load(&id))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn callback_query_keeps_the_first_bounded_code_and_state() {
        let parsed = parse_callback(Some("state=s&code=c&code=other&scope=openid&authuser=0"));
        assert_eq!(parsed.state.as_deref(), Some("s"));
        assert_eq!(parsed.code.as_deref(), Some("c"));
        assert!(!parsed.denied);
        assert!(parse_callback(Some("error=access_denied&state=s")).denied);
        assert_eq!(parse_callback(None), CallbackQuery::default());
        let long = format!("code={}", "a".repeat(5000));
        assert_eq!(parse_callback(Some(&long)).code, None);
    }

    #[test]
    fn notices_have_fixed_messages_and_unknown_codes_are_generic() {
        for notice in [
            Notice::Cancelled,
            Notice::Invalid,
            Notice::Failed,
            Notice::Unavailable,
        ] {
            assert_ne!(Notice::message(notice.code()), "");
        }
        assert_eq!(Notice::message("<script>"), Notice::message("failed"));
    }

    #[test]
    fn sign_in_routes_are_public_and_others_are_not() {
        assert!(public(START.path()) && public(CALLBACK) && public(LOGIN.path()));
        assert!(!public("/auth/logout") && !public(crate::INDEX.path()));
    }
}
