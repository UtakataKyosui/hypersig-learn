//! Sign-in with Google through OIDC (authorization code with PKCE).
//!
//! The issuer is fixed. Every request this module makes goes through
//! [`Fetch`], which refuses redirects and any host other than Google's.
//! Discovery fetches the signing keys together with the metadata, and each
//! sign-in discovers again, so no key is cached across requests: a rotated key
//! is read on the next sign-in, and a key Google withdrew stops being trusted.
//! `openidconnect` verifies the signature, `iss`, `aud`, `exp` and `nonce`; it
//! does not check `azp`, so [`verify`] does.
use chrono::{DateTime, Utc};
use openidconnect::core::{
    CoreAuthPrompt, CoreAuthenticationFlow, CoreClient, CoreIdToken, CoreIdTokenVerifier,
    CoreJsonWebKeySet, CoreJwsSigningAlgorithm, CoreProviderMetadata,
};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, HttpRequest, HttpResponse, IssuerUrl,
    Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, TokenResponse as _,
};
use std::{future::Future, pin::Pin};
use worker::send::SendFuture;

pub const ISSUER: &str = "https://accounts.google.com";
/// Hosts the module may fetch from: discovery and sign-in, the token
/// endpoint, and the key set.
const HOSTS: [&str; 3] = [
    "accounts.google.com",
    "oauth2.googleapis.com",
    "www.googleapis.com",
];
const MAX_RESPONSE: usize = 256 * 1024;

/// Why a step failed. The text never carries a token, a code or a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Config,
    Http,
    Provider,
    Exchange,
    Token,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Config => "sign-in is not configured",
            Self::Http => "request to the identity provider was refused or failed",
            Self::Provider => "identity provider metadata is not acceptable",
            Self::Exchange => "authorization code was not accepted",
            Self::Token => "ID token was not accepted",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Clone)]
pub struct Config {
    client_id: String,
    client_secret: String,
    redirect_url: String,
}
impl Config {
    /// The redirect URL is configuration, never taken from a request. It must
    /// be HTTPS, or loopback HTTP for local development.
    pub fn new(client_id: &str, client_secret: &str, redirect_url: &str) -> Result<Self, Error> {
        let url = url::Url::parse(redirect_url).map_err(|_| Error::Config)?;
        let loopback = url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        let acceptable = (url.scheme() == "https" || loopback)
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none();
        if client_id.is_empty() || client_secret.is_empty() || !acceptable {
            return Err(Error::Config);
        }
        Ok(Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            redirect_url: redirect_url.into(),
        })
    }
}

/// One HTTP request, which is all [`Fetch`] and a test double provide.
pub type Pending<'c> = Pin<Box<dyn Future<Output = Result<HttpResponse, Error>> + Send + 'c>>;

/// Whether the module may send `request`: HTTPS, a Google host and the
/// default port.
fn allowed(request: &HttpRequest) -> bool {
    let uri = request.uri();
    uri.scheme_str() == Some("https")
        && uri.port().is_none()
        && uri
            .authority()
            .is_some_and(|authority| !authority.as_str().contains('@'))
        && uri.host().is_some_and(|host| HOSTS.contains(&host))
}

/// The Workers `fetch`, restricted by [`allowed`], that never follows a
/// redirect and reads at most [`MAX_RESPONSE`] bytes.
pub struct Fetch;
impl<'c> openidconnect::AsyncHttpClient<'c> for Fetch {
    type Error = Error;
    type Future = Pending<'c>;
    fn call(&'c self, request: HttpRequest) -> Self::Future {
        Box::pin(SendFuture::new(send(request)))
    }
}

async fn send(request: HttpRequest) -> Result<HttpResponse, Error> {
    if !allowed(&request) {
        return Err(Error::Http);
    }
    let headers = worker::Headers::new();
    for (name, value) in request.headers() {
        let value = value.to_str().map_err(|_| Error::Http)?;
        headers
            .append(name.as_str(), value)
            .map_err(|_| Error::Http)?;
    }
    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::from(request.method().as_str().to_owned()))
        .with_headers(headers)
        .with_redirect(worker::RequestRedirect::Manual);
    if !request.body().is_empty() {
        init.with_body(Some(
            js_sys::Uint8Array::from(request.body().as_slice()).into(),
        ));
    }
    let outgoing = worker::Request::new_with_init(&request.uri().to_string(), &init)
        .map_err(|_| Error::Http)?;
    let mut response = worker::Fetch::Request(outgoing)
        .send()
        .await
        .map_err(|_| Error::Http)?;
    let status = response.status_code();
    if (300..400).contains(&status) {
        return Err(Error::Http);
    }
    let body = response.bytes().await.map_err(|_| Error::Http)?;
    if body.len() > MAX_RESPONSE {
        return Err(Error::Http);
    }
    let mut builder = openidconnect::http::Response::builder().status(status);
    if let Ok(Some(content_type)) = response.headers().get("content-type") {
        builder = builder.header("content-type", content_type);
    }
    builder.body(body).map_err(|_| Error::Http)
}

async fn discover<C>(client: &C) -> Result<CoreProviderMetadata, Error>
where
    C: for<'c> openidconnect::AsyncHttpClient<'c, Error = Error>,
{
    let issuer = IssuerUrl::new(ISSUER.into()).map_err(|_| Error::Config)?;
    let metadata = CoreProviderMetadata::discover_async(issuer, client)
        .await
        .map_err(|_| Error::Provider)?;
    // The page sends the visitor to this endpoint, so it must be Google's too.
    let authorization = metadata.authorization_endpoint().url();
    if authorization.scheme() != "https"
        || authorization.host_str() != Some("accounts.google.com")
        || metadata.token_endpoint().is_none()
    {
        return Err(Error::Provider);
    }
    Ok(metadata)
}

/// Where to send the visitor, and what to remember until they come back.
pub struct Started {
    pub url: url::Url,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
}

/// Builds the authorization request: code flow, PKCE S256, the `openid`
/// scope only, and a state and nonce that are 128 random bits each.
pub async fn start<C>(client: &C, config: &Config) -> Result<Started, Error>
where
    C: for<'c> openidconnect::AsyncHttpClient<'c, Error = Error>,
{
    let metadata = discover(client).await?;
    let oidc = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(config.client_id.clone()),
        Some(ClientSecret::new(config.client_secret.clone())),
    )
    .set_redirect_uri(RedirectUrl::new(config.redirect_url.clone()).map_err(|_| Error::Config)?);
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, state, nonce) = oidc
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .set_pkce_challenge(challenge)
        .add_prompt(CoreAuthPrompt::SelectAccount)
        .url();
    Ok(Started {
        url,
        state: state.secret().clone(),
        nonce: nonce.secret().clone(),
        pkce_verifier: verifier.secret().clone(),
    })
}

/// The identity Google vouched for. `(issuer, subject)` is the only identity
/// the application keeps; a mail address is never used to recognise a person.
#[derive(Debug, PartialEq, Eq)]
pub struct Verified {
    pub issuer: String,
    pub subject: String,
}

/// Exchanges the code and verifies the ID token against freshly fetched keys.
pub async fn finish<C>(
    client: &C,
    config: &Config,
    code: &str,
    pkce_verifier: &str,
    nonce: &str,
    now: DateTime<Utc>,
) -> Result<Verified, Error>
where
    C: for<'c> openidconnect::AsyncHttpClient<'c, Error = Error>,
{
    let metadata = discover(client).await?;
    let jwks = metadata.jwks().clone();
    let oidc = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(config.client_id.clone()),
        Some(ClientSecret::new(config.client_secret.clone())),
    )
    .set_redirect_uri(RedirectUrl::new(config.redirect_url.clone()).map_err(|_| Error::Config)?);
    let response = oidc
        .exchange_code(AuthorizationCode::new(code.into()))
        .map_err(|_| Error::Provider)?
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier.into()))
        .request_async(client)
        .await
        .map_err(|_| Error::Exchange)?;
    let token = response.id_token().ok_or(Error::Token)?;
    verify(
        token,
        &config.client_id,
        jwks,
        &Nonce::new(nonce.into()),
        now,
    )
}

/// Checks the ID token: RS256 signature by a key of the set, `iss`, `aud`,
/// `exp` and `nonce` by `openidconnect`, and `azp` here.
pub fn verify(
    token: &CoreIdToken,
    client_id: &str,
    jwks: CoreJsonWebKeySet,
    nonce: &Nonce,
    now: DateTime<Utc>,
) -> Result<Verified, Error> {
    let verifier = CoreIdTokenVerifier::new_public_client(
        ClientId::new(client_id.into()),
        IssuerUrl::new(ISSUER.into()).map_err(|_| Error::Config)?,
        jwks,
    )
    .set_allowed_algs([CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256])
    .set_time_fn(move || now);
    let claims = token.claims(&verifier, nonce).map_err(|_| Error::Token)?;
    let authorized_party = claims.authorized_party().map(|party| party.as_str());
    let party_ok = match authorized_party {
        Some(party) => party == client_id,
        None => claims.audiences().len() == 1,
    };
    let subject = claims.subject().as_str();
    if !party_ok || claims.issuer().as_str() != ISSUER || subject.is_empty() || subject.len() > 255
    {
        return Err(Error::Token);
    }
    Ok(Verified {
        issuer: ISSUER.into(),
        subject: subject.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidconnect::core::{CoreIdTokenClaims, CoreRsaPrivateSigningKey};
    use openidconnect::{
        Audience, EmptyAdditionalClaims, JsonWebKeyId, StandardClaims, SubjectIdentifier,
    };
    use std::cell::RefCell;

    /// Two RSA keys made for this run, which sign test tokens only. A small
    /// modulus keeps debug builds fast.
    fn test_keys() -> &'static (String, String) {
        use rsa::pkcs1::{EncodeRsaPrivateKey as _, LineEnding};
        static KEYS: std::sync::OnceLock<(String, String)> = std::sync::OnceLock::new();
        KEYS.get_or_init(|| {
            let pem = || {
                rsa::RsaPrivateKey::new(&mut rand::rngs::OsRng, 1024)
                    .unwrap()
                    .to_pkcs1_pem(LineEnding::LF)
                    .unwrap()
                    .to_string()
            };
            (pem(), pem())
        })
    }
    const CLIENT: &str = "client-id.apps.example";
    const NONCE: &str = "the-nonce";

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000, 0).unwrap()
    }
    fn signer(pem: &str, kid: &str) -> CoreRsaPrivateSigningKey {
        CoreRsaPrivateSigningKey::from_pem(pem, Some(JsonWebKeyId::new(kid.into()))).unwrap()
    }
    fn jwks(keys: &[&CoreRsaPrivateSigningKey]) -> CoreJsonWebKeySet {
        use openidconnect::PrivateSigningKey as _;
        CoreJsonWebKeySet::new(keys.iter().map(|key| key.as_verification_key()).collect())
    }

    struct Claims {
        issuer: &'static str,
        audience: &'static str,
        nonce: &'static str,
        azp: Option<&'static str>,
        subject: &'static str,
        expires_in: i64,
        algorithm: CoreJwsSigningAlgorithm,
    }
    impl Default for Claims {
        fn default() -> Self {
            Self {
                issuer: ISSUER,
                audience: CLIENT,
                nonce: NONCE,
                azp: Some(CLIENT),
                subject: "1234567890",
                expires_in: 600,
                algorithm: CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
            }
        }
    }
    impl Claims {
        fn sign(&self, key: &CoreRsaPrivateSigningKey) -> CoreIdToken {
            let claims = CoreIdTokenClaims::new(
                IssuerUrl::new(self.issuer.into()).unwrap(),
                vec![Audience::new(self.audience.into())],
                now() + chrono::Duration::seconds(self.expires_in),
                now() - chrono::Duration::seconds(10),
                StandardClaims::new(SubjectIdentifier::new(self.subject.into())),
                EmptyAdditionalClaims {},
            )
            .set_nonce(Some(Nonce::new(self.nonce.into())))
            .set_authorized_party(self.azp.map(|azp| ClientId::new(azp.into())));
            CoreIdToken::new(claims, key, self.algorithm.clone(), None, None).unwrap()
        }
    }

    fn check(token: &CoreIdToken, keys: CoreJsonWebKeySet) -> Result<Verified, Error> {
        verify(token, CLIENT, keys, &Nonce::new(NONCE.into()), now())
    }

    #[test]
    fn accepts_a_valid_token_and_returns_issuer_and_subject() {
        let key = signer(&test_keys().0, "k1");
        let verified = check(&Claims::default().sign(&key), jwks(&[&key])).unwrap();
        assert_eq!(
            verified,
            Verified {
                issuer: ISSUER.into(),
                subject: "1234567890".into()
            }
        );
    }

    #[test]
    fn rejects_every_wrong_claim_signature_and_algorithm() {
        let key = signer(&test_keys().0, "k1");
        let keys = || jwks(&[&key]);
        let rejected = |claims: Claims| check(&claims.sign(&key), keys()) == Err(Error::Token);
        assert!(rejected(Claims {
            issuer: "https://evil.example",
            ..Claims::default()
        }));
        assert!(rejected(Claims {
            audience: "other-client",
            azp: None,
            ..Claims::default()
        }));
        assert!(rejected(Claims {
            nonce: "replayed",
            ..Claims::default()
        }));
        assert!(rejected(Claims {
            azp: Some("other-client"),
            ..Claims::default()
        }));
        assert!(rejected(Claims {
            expires_in: -1,
            ..Claims::default()
        }));
        assert!(rejected(Claims {
            subject: "",
            ..Claims::default()
        }));
        assert!(rejected(Claims {
            algorithm: CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha384,
            ..Claims::default()
        }));
        // Signed by another key that carries the same key id.
        let forger = signer(&test_keys().1, "k1");
        assert_eq!(
            check(&Claims::default().sign(&forger), keys()),
            Err(Error::Token)
        );
    }

    #[test]
    fn a_missing_azp_is_accepted_for_a_single_audience() {
        let key = signer(&test_keys().0, "k1");
        let token = Claims {
            azp: None,
            ..Claims::default()
        }
        .sign(&key);
        assert!(check(&token, jwks(&[&key])).is_ok());
    }

    #[test]
    fn key_rotation_needs_the_new_key_in_the_fetched_set() {
        let old = signer(&test_keys().0, "old");
        let new = signer(&test_keys().1, "new");
        let token = Claims::default().sign(&new);
        // A set without the new key id, as a stale copy would be.
        assert_eq!(check(&token, jwks(&[&old])), Err(Error::Token));
        assert!(check(&token, jwks(&[&old, &new])).is_ok());
        // A withdrawn key stops verifying once the set no longer lists it.
        let retired = Claims::default().sign(&old);
        assert_eq!(check(&retired, jwks(&[&new])), Err(Error::Token));
    }

    #[test]
    fn requests_are_limited_to_https_google_hosts() {
        let request = |uri: &str| {
            openidconnect::http::Request::get(uri)
                .body(Vec::new())
                .unwrap()
        };
        for uri in [
            "https://accounts.google.com/.well-known/openid-configuration",
            "https://oauth2.googleapis.com/token",
            "https://www.googleapis.com/oauth2/v3/certs",
        ] {
            assert!(allowed(&request(uri)), "{uri}");
        }
        for uri in [
            "http://accounts.google.com/",
            "https://accounts.google.com:8443/",
            "https://evil.example/",
            "https://accounts.google.com.evil.example/",
            "https://user@accounts.google.com/",
            "https://169.254.169.254/",
            "/relative",
        ] {
            assert!(!allowed(&request(uri)), "{uri}");
        }
    }

    #[test]
    fn config_accepts_only_https_or_loopback_redirects() {
        let config = |redirect| Config::new("id", "secret", redirect);
        assert!(config("https://app.example/auth/callback").is_ok());
        assert!(config("http://localhost:8787/auth/callback").is_ok());
        for redirect in [
            "http://app.example/auth/callback",
            "javascript:alert(1)",
            "https://u:p@app.example/cb",
            "https://app.example/cb#frag",
            "not a url",
        ] {
            assert_eq!(config(redirect).err(), Some(Error::Config), "{redirect}");
        }
        assert_eq!(
            Config::new("", "secret", "https://app.example/cb").err(),
            Some(Error::Config)
        );
    }

    /// Answers by URL and records every request, in place of Google.
    struct Fake {
        routes: Vec<(&'static str, String)>,
        seen: RefCell<Vec<(String, String)>>,
    }
    impl<'c> openidconnect::AsyncHttpClient<'c> for Fake {
        type Error = Error;
        type Future = Pending<'c>;
        fn call(&'c self, request: HttpRequest) -> Self::Future {
            let uri = request.uri().to_string();
            let body = String::from_utf8_lossy(request.body()).into_owned();
            self.seen.borrow_mut().push((uri.clone(), body));
            let found = self
                .routes
                .iter()
                .find(|(prefix, _)| uri.starts_with(prefix))
                .map(|(_, body)| body.clone());
            Box::pin(async move {
                let body = found.ok_or(Error::Http)?;
                openidconnect::http::Response::builder()
                    .status(200)
                    .header("content-type", "application/json")
                    .body(body.into_bytes())
                    .map_err(|_| Error::Http)
            })
        }
    }

    fn google(
        authorization: &str,
        keys: &CoreJsonWebKeySet,
        id_token: Option<&CoreIdToken>,
    ) -> Fake {
        let metadata = serde_json::json!({
            "issuer": ISSUER,
            "authorization_endpoint": authorization,
            "token_endpoint": "https://oauth2.googleapis.com/token",
            "jwks_uri": "https://www.googleapis.com/oauth2/v3/certs",
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
        });
        let mut routes = vec![
            (
                "https://accounts.google.com/.well-known/openid-configuration",
                metadata.to_string(),
            ),
            (
                "https://www.googleapis.com/oauth2/v3/certs",
                serde_json::to_string(keys).unwrap(),
            ),
        ];
        if let Some(token) = id_token {
            routes.push((
                "https://oauth2.googleapis.com/token",
                serde_json::json!({
                    "access_token": "access",
                    "token_type": "Bearer",
                    "id_token": serde_json::to_value(token).unwrap(),
                })
                .to_string(),
            ));
        }
        Fake {
            routes,
            seen: RefCell::default(),
        }
    }
    fn config() -> Config {
        Config::new(CLIENT, "secret", "https://app.example/auth/callback").unwrap()
    }
    const AUTHORIZATION: &str = "https://accounts.google.com/o/oauth2/v2/auth";

    #[test]
    fn start_builds_a_pkce_code_request_with_the_fixed_redirect() {
        let key = signer(&test_keys().0, "k1");
        let fake = google(AUTHORIZATION, &jwks(&[&key]), None);
        let started = futures::executor::block_on(start(&fake, &config())).unwrap();
        let query: std::collections::HashMap<_, _> =
            started.url.query_pairs().into_owned().collect();
        assert_eq!(started.url.host_str(), Some("accounts.google.com"));
        assert_eq!(query["response_type"], "code");
        assert_eq!(query["client_id"], CLIENT);
        assert_eq!(query["redirect_uri"], "https://app.example/auth/callback");
        assert_eq!(query["scope"], "openid");
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(query["state"], started.state);
        assert_eq!(query["nonce"], started.nonce);
        assert_ne!(query["code_challenge"], started.pkce_verifier);
        assert!(started.state.len() >= 22 && started.nonce.len() >= 22);
    }

    #[test]
    fn start_refuses_an_authorization_endpoint_outside_google() {
        let key = signer(&test_keys().0, "k1");
        let fake = google("https://evil.example/auth", &jwks(&[&key]), None);
        let result = futures::executor::block_on(start(&fake, &config()));
        assert_eq!(result.err(), Some(Error::Provider));
    }

    #[test]
    fn finish_exchanges_the_code_with_the_verifier_and_verifies_the_token() {
        let key = signer(&test_keys().0, "k1");
        let token = Claims::default().sign(&key);
        let fake = google(AUTHORIZATION, &jwks(&[&key]), Some(&token));
        let verified = futures::executor::block_on(finish(
            &fake,
            &config(),
            "the-code",
            "the-verifier",
            NONCE,
            now(),
        ))
        .unwrap();
        assert_eq!(verified.subject, "1234567890");
        let seen = fake.seen.borrow();
        let exchange = &seen
            .iter()
            .find(|(uri, _)| uri.ends_with("/token"))
            .unwrap()
            .1;
        assert!(exchange.contains("grant_type=authorization_code"));
        assert!(exchange.contains("code=the-code"));
        assert!(exchange.contains("code_verifier=the-verifier"));
        assert!(exchange.contains("redirect_uri=https%3A%2F%2Fapp.example%2Fauth%2Fcallback"));
        // Keys are fetched on every sign-in, never reused from an earlier one.
        assert_eq!(
            seen.iter()
                .filter(|(uri, _)| uri.contains("/certs"))
                .count(),
            1
        );
    }

    #[test]
    fn finish_rejects_a_wrong_nonce_and_a_failed_exchange() {
        let key = signer(&test_keys().0, "k1");
        let token = Claims::default().sign(&key);
        let fake = google(AUTHORIZATION, &jwks(&[&key]), Some(&token));
        let run = |fake: &Fake, nonce| {
            futures::executor::block_on(finish(fake, &config(), "c", "v", nonce, now()))
        };
        assert_eq!(run(&fake, "other").err(), Some(Error::Token));
        let no_token = google(AUTHORIZATION, &jwks(&[&key]), None);
        assert_eq!(run(&no_token, NONCE).err(), Some(Error::Exchange));
    }

    #[test]
    fn errors_name_no_secret() {
        for error in [
            Error::Config,
            Error::Http,
            Error::Provider,
            Error::Exchange,
            Error::Token,
        ] {
            let text = error.to_string();
            assert!(!text.contains("secret") && !text.contains("eyJ"));
        }
    }
}
