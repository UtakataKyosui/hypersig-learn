//! Sign-in with Google, and the `Member` every protected handler requires.
//!
//! The browser holds one cookie with a session id. The session record, the
//! user and the OAuth flows live in D1 (`auth_store`, `session_store`).
use crate::{AppState, assets, auth_store, oidc, session_state};
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{HeaderValue, StatusCode, Uri, header, request::Parts},
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

/// A page navigation goes to the login page; everything else, including an
/// action or stream the runtime fetches, is refused with 401.
fn refusal(parts: &Parts) -> Response {
    if parts.method == axum::http::Method::GET
        && RequestKind::of(parts) == RequestKind::Navigation
        && [
            crate::INDEX.path(),
            crate::MEDICATIONS.path(),
            crate::NOTIFICATIONS.path(),
        ]
        .contains(&parts.uri.path())
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
hypersig::routes! {
    fn routes(Router<AppState>);
    const LOGIN: Route = "/login" => page;
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
    use hypersig::html::{a, div, p};
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
    .child(
        div()
            .child(a().href(START.link()).text("Google でログイン"))
            .child(p().bind_text(Login::problem())),
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
    let _ = session.insert(NOTICE, notice.code()).await;
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
async fn start(state: AppState, session: Session, (): ()) -> Result<PageResponse, Response> {
    pin(&session).await;
    let now = auth_store::now_ms();
    let flow = async {
        let started = oidc::start(&oidc::Fetch, &state.oidc).await.ok()?;
        let binding = auth_store::random_token().ok()?;
        auth_store::create_flow(
            &state.db,
            &started.state,
            &started.nonce,
            &started.pkce_verifier,
            &binding,
            now + FLOW_LIFETIME_MS,
        )
        .await
        .ok()?;
        session.insert(BINDING, binding).await.ok()?;
        Some(started.url)
    }
    .await;
    // Expired flows and sessions are removed as visitors arrive, and by cron.
    let _ = auth_store::purge_expired(&state.db, now).await;
    if let Some(url) = flow {
        return Err(private(Redirect::to(url.as_str()).into_response()));
    }
    log_failure("start");
    let _ = session.insert(NOTICE, Notice::Failed.code()).await;
    Ok(PageResponse::redirect(LOGIN.url(())))
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
