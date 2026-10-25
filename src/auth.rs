//! Single-owner, expiring signed cookies. Rotating `AUTH_SECRET` revokes all sessions.
use crate::{AppState, assets};
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{StatusCode, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use hypersig::Route;
use hypersig::{Action, Page, Patches, RequestKind, Signals};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

const COOKIE: &str = "__Host-medication";
const LIFETIME: u64 = 86_400;
#[derive(Clone)]
pub struct Config {
    pub secret: String,
    pub password: String,
}
impl Config {
    fn mac(&self, value: &[u8]) -> Hmac<Sha256> {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.secret.as_bytes())
            .expect("HMAC accepts any key length");
        mac.update(self.password.as_bytes());
        mac.update(&[0]);
        mac.update(value);
        mac
    }
    fn token(&self, now: u64) -> String {
        let payload = format!("owner:{}", now + LIFETIME);
        format!(
            "{payload}.{}",
            URL_SAFE_NO_PAD.encode(self.mac(payload.as_bytes()).finalize().into_bytes())
        )
    }
    fn valid(&self, token: &str, now: u64) -> bool {
        let Some((payload, signature)) = token.split_once('.') else {
            return false;
        };
        let Some(expiry) = payload
            .strip_prefix("owner:")
            .and_then(|s| s.parse::<u64>().ok())
        else {
            return false;
        };
        expiry > now
            && expiry <= now.saturating_add(LIFETIME)
            && URL_SAFE_NO_PAD.decode(signature).is_ok_and(|signature| {
                self.mac(payload.as_bytes())
                    .verify_slice(&signature)
                    .is_ok()
            })
    }
}
fn now() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        worker::Date::now().as_millis() / 1000
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_secs()
    }
}
#[derive(Clone)]
pub struct Member(&'static str);
impl Member {
    pub fn owner(&self) -> &'static str {
        self.0
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
pub async fn protect(State(config): State<Config>, mut request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let public = [
        assets::MANIFEST,
        assets::SERVICE_WORKER,
        assets::SUBSCRIBE_SCRIPT,
        assets::ICON_192,
        assets::ICON_512,
        hypersig::RUNTIME_PATH,
        LOGIN.path(),
        SIGN_IN.path(),
    ];
    // Runtime assets contain no user data.
    if public.contains(&path) {
        return next.run(request).await;
    }
    let authenticated = request
        .headers()
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|cookies| {
            cookies
                .split(';')
                .filter_map(|c| c.trim().split_once('='))
                .any(|(name, value)| name == COOKIE && config.valid(value, now()))
        });
    if authenticated {
        request.extensions_mut().insert(Member("owner"));
        let mut response = next.run(request).await;
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-store"),
        );
        return response;
    }
    let (parts, _) = request.into_parts();
    if parts.method == axum::http::Method::GET
        && RequestKind::of(&parts) == RequestKind::Navigation
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
#[derive(Signals, Serialize, Deserialize)]
pub struct Login {
    password: String,
    problem: String,
}
pub const SIGN_IN: Action<Login, String> = Action::post("/auth/login");
pub const SIGN_OUT: Action<Login, ()> = Action::post("/auth/logout");
hypersig::routes! { fn routes(Router<AppState>); const LOGIN: Route = "/login" => page; }
async fn page(_: AppState, (): ()) -> Page<Login> {
    use hypersig::html::{div, p};
    Page::new(
        "ログイン",
        Login {
            password: String::new(),
            problem: String::new(),
        },
    )
    .child(
        div()
            .child(
                hypersig_ui::TextField::new("パスワード", Login::password())
                    .kind(hypersig_ui::InputKind::Password),
            )
            .child(
                hypersig_ui::button(
                    hypersig_ui::ButtonVariant::Default,
                    hypersig_ui::ButtonSize::Default,
                )
                .text("ログイン")
                .on_click(SIGN_IN.call_with(Login::password()).error(Login::problem())),
            )
            .child(p().bind_text(Login::problem())),
    )
}
async fn sign_in(
    state: AppState,
    password: String,
) -> Result<([(header::HeaderName, String); 1], Patches<Login>), StatusCode> {
    // Compare fixed-size MACs using the MAC crate's constant-time verifier.
    if password.len() > 1024 {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let expected = state
        .auth
        .mac(state.auth.password.as_bytes())
        .finalize()
        .into_bytes();
    if state
        .auth
        .mac(password.as_bytes())
        .verify_slice(&expected)
        .is_err()
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok((
        [(
            header::SET_COOKIE,
            format!(
                "{COOKIE}={}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={LIFETIME}",
                state.auth.token(now())
            ),
        )],
        Patches::new().navigate(crate::INDEX.url(())),
    ))
}
async fn sign_out(
    _: AppState,
    _: Member,
    (): (),
) -> ([(header::HeaderName, String); 1], Patches<Login>) {
    (
        [(
            header::SET_COOKIE,
            format!("{COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0"),
        )],
        Patches::new().navigate(LOGIN.url(())),
    )
}
pub fn router(router: axum::Router<AppState>) -> axum::Router<AppState> {
    use hypersig::RouterExt as _;
    routes(router)
        .action(SIGN_IN, sign_in)
        .action(SIGN_OUT, sign_out)
}
#[cfg(test)]
mod tests {
    use super::*;
    use hypersig::RouterExt as _;
    use tower::ServiceExt as _;
    fn config() -> Config {
        Config {
            secret: "test secret with at least 32 characters".into(),
            password: "test-password".into(),
        }
    }
    async fn accepted((): (), _: Member, _: String) -> Patches<Login> {
        Login::problem().patch("accepted".into()).into()
    }
    fn router(protected: bool) -> axum::Router {
        let router = axum::Router::new().action(SIGN_IN, accepted);
        if protected {
            router.layer(axum::middleware::from_fn_with_state(config(), protect))
        } else {
            router
        }
    }
    fn status(request: Request, protected: bool) -> StatusCode {
        futures::executor::block_on(router(protected).oneshot(request))
            .unwrap()
            .status()
    }
    #[test]
    fn router_rejects_foreign_content_and_missing_member() {
        assert_eq!(
            status(SIGN_IN.request(&String::new()), false),
            StatusCode::UNAUTHORIZED
        );
        let mut request = SIGN_IN.request(&String::new());
        request
            .headers_mut()
            .insert("sec-fetch-site", "cross-site".parse().unwrap());
        assert_eq!(status(request, false), StatusCode::FORBIDDEN);
        let mut request = SIGN_IN.request(&String::new());
        request
            .headers_mut()
            .insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
        assert_eq!(status(request, false), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        let mut request = SIGN_IN.request(&String::new());
        request.extensions_mut().insert(Member("owner"));
        *request.body_mut() = axum::body::Body::from("{invalid");
        assert_eq!(status(request, false), StatusCode::BAD_REQUEST);
        let mut request = SIGN_IN.request(&String::new());
        request.extensions_mut().insert(Member("owner"));
        assert_eq!(status(request, false), StatusCode::OK);
    }
    #[test]
    fn middleware_redirects_pages_and_refuses_direct_actions_and_streams() {
        let app = || {
            axum::Router::new()
                .route(
                    crate::MEDICATIONS.path(),
                    axum::routing::get(|| async { "private" }),
                )
                .layer(axum::middleware::from_fn_with_state(config(), protect))
        };
        let request = Request::builder()
            .uri(crate::MEDICATIONS.path())
            .header(header::ACCEPT, "text/html")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = futures::executor::block_on(app().oneshot(request)).unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[header::LOCATION], LOGIN.path());
        let stream: hypersig::StreamAction<Login> = hypersig::StreamAction::new("/medications");
        let response = futures::executor::block_on(app().oneshot(stream.request(&()))).unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let action: Action<Login, ()> = Action::post("/medications");
        let response = futures::executor::block_on(app().oneshot(action.request(&()))).unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let request = Request::builder()
            .uri(crate::MEDICATIONS.path())
            .header(
                header::COOKIE,
                format!("{COOKIE}={}", config().token(now())),
            )
            .body(axum::body::Body::empty())
            .unwrap();
        let response = futures::executor::block_on(app().oneshot(request)).unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
    #[test]
    fn expiry_tampering_and_rotation() {
        let config = Config {
            secret: "a sufficiently long secret".into(),
            password: "password".into(),
        };
        let token = config.token(100);
        assert!(config.valid(&token, 101));
        assert!(!config.valid(&token, 100 + LIFETIME));
        assert!(!config.valid(&format!("{token}x"), 101));
        assert!(
            !Config {
                secret: "rotated".into(),
                ..config
            }
            .valid(&token, 101)
        );
    }
}
