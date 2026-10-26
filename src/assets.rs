//! The files a browser fetches without a page: the Service Worker, the
//! script that defines `<push-subscribe>`, the web app manifest, and the
//! icons. Each path is written here once, and the page, the manifest, and
//! the router all read it from these constants.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

pub const SERVICE_WORKER: &str = "/sw.js";
pub const SUBSCRIBE_SCRIPT: &str = "/push-subscribe.js";
pub const MANIFEST: &str = "/manifest.webmanifest";
pub const ICON_192: &str = "/icon-192.png";
pub const ICON_512: &str = "/icon-512.png";

/// The scope the Service Worker is registered with. `Service-Worker-Allowed`
/// on the script response lets a worker served from `/sw.js` cover it.
pub const SCOPE: &str = "/";

const THEME_COLOR: &str = "#2563eb";
const HEAD_LINKS_LIMIT: usize = 1 << 20;

const SERVICE_WORKER_JS: &str = include_str!("../assets/sw.js");
const SUBSCRIBE_JS: &str = include_str!("../assets/push-subscribe.js");
const ICON_192_PNG: &[u8] = include_bytes!("../assets/icon-192.png");
const ICON_512_PNG: &[u8] = include_bytes!("../assets/icon-512.png");

fn manifest_json(start_url: &str) -> String {
    serde_json::json!({
        "name": "服薬リマインダー",
        "short_name": "服薬",
        "start_url": start_url,
        "scope": SCOPE,
        "display": "standalone",
        "background_color": "#ffffff",
        "theme_color": THEME_COLOR,
        "icons": [
            { "src": ICON_192, "sizes": "192x192", "type": "image/png" },
            { "src": ICON_512, "sizes": "512x512", "type": "image/png" },
        ],
    })
    .to_string()
}

fn asset(content_type: &'static str, body: impl Into<Body>) -> Response {
    let mut response = Response::new(body.into());
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
}

/// Adds the asset routes to `router`.
pub fn routes<S: Clone + Send + Sync + 'static>(router: Router<S>) -> Router<S> {
    router
        .route(
            SERVICE_WORKER,
            get(|| async {
                let config = serde_json::json!({"today":crate::INDEX.url(()).as_str()});
                let mut response = asset(
                    "text/javascript; charset=utf-8",
                    format!("const appRoutes = {config};\n{SERVICE_WORKER_JS}"),
                );
                let headers = response.headers_mut();
                headers.insert("service-worker-allowed", HeaderValue::from_static(SCOPE));
                headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
                response
            }),
        )
        .route(
            SUBSCRIBE_SCRIPT,
            get(|| async { asset("text/javascript; charset=utf-8", SUBSCRIBE_JS) }),
        )
        .route(
            MANIFEST,
            get(|| async {
                asset(
                    "application/manifest+json",
                    manifest_json(crate::NOTIFICATIONS.url(()).as_str()),
                )
            }),
        )
        .route(ICON_192, get(|| async { asset("image/png", ICON_192_PNG) }))
        .route(ICON_512, get(|| async { asset("image/png", ICON_512_PNG) }))
}

/// The tags `hypersig::Page` 0.4.1 has no way to put in `<head>`.
fn head_links() -> String {
    format!(
        r#"<link rel="manifest" href="{MANIFEST}"><link rel="apple-touch-icon" href="{ICON_192}"><meta name="theme-color" content="{THEME_COLOR}"></head>"#
    )
}

/// Puts the manifest link before the first `</head>` of `html`. Returns
/// `None` if there is none, so a body that is not a page is left alone.
fn insert_head_links(html: &str) -> Option<String> {
    let at = html.find("</head>")?;
    Some(format!(
        "{}{}{}",
        &html[..at],
        head_links(),
        &html[at + "</head>".len()..]
    ))
}

/// Middleware for the router: puts the manifest link in the `<head>` of each
/// HTML page and keeps the rest of the response, including its headers.
/// `Content-Length` is dropped because the body length changes.
pub async fn add_head_links(response: Response) -> Response {
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));
    if !is_html {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let Ok(bytes) = to_bytes(body, HEAD_LINKS_LIMIT).await else {
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "page too large",
        )
            .into_response();
    };
    let body = match std::str::from_utf8(&bytes).ok().and_then(insert_head_links) {
        Some(html) => Body::from(html),
        None => Body::from(bytes),
    };
    parts.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(parts, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_scope_covers_start_url_and_names_the_icons() {
        let manifest: serde_json::Value =
            serde_json::from_str(&manifest_json(crate::NOTIFICATIONS.url(()).as_str())).unwrap();
        assert_eq!(manifest["scope"], SCOPE);
        assert!(manifest["start_url"].as_str().unwrap().starts_with(SCOPE));
        assert_eq!(manifest["display"], "standalone");
        assert_eq!(manifest["icons"][0]["src"], ICON_192);
        assert_eq!(manifest["icons"][1]["src"], ICON_512);
    }

    #[test]
    fn inserts_links_once_before_the_head_closes() {
        let html = "<html><head><title>x</title></head><body></body></html>";
        let out = insert_head_links(html).unwrap();
        assert_eq!(out.matches(r#"rel="manifest""#).count(), 1);
        assert!(out.contains(&format!(r#"href="{MANIFEST}"><link"#)));
        assert!(out.find("rel=\"manifest\"").unwrap() < out.find("</head>").unwrap());
        assert!(out.ends_with("</body></html>"));
    }

    #[test]
    fn leaves_a_body_without_a_head_alone() {
        assert_eq!(insert_head_links("plain"), None);
    }

    #[test]
    fn keeps_headers_but_not_content_length() {
        let mut response = Response::new(Body::from("<head></head>"));
        let headers = response.headers_mut();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("13"));
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'self'"),
        );
        let out = futures::executor::block_on(add_head_links(response));
        assert!(out.headers().get(header::CONTENT_LENGTH).is_none());
        assert_eq!(
            out.headers()[header::CONTENT_SECURITY_POLICY],
            "default-src 'self'"
        );
        let body = futures::executor::block_on(to_bytes(out.into_body(), 4096)).unwrap();
        assert!(
            String::from_utf8(body.to_vec())
                .unwrap()
                .contains("rel=\"manifest\"")
        );
    }
}
