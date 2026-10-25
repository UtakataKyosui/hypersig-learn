//! Web Push for a worker: the message encryption of RFC 8291 (`aes128gcm`
//! content coding of RFC 8188) and the VAPID token of RFC 8292, built on
//! `RustCrypto` crates that compile to `wasm32-unknown-unknown`, and the HTTP
//! request that hands the message to the push service.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use hkdf::Hkdf;
use p256::ecdh::diffie_hellman;
use p256::ecdsa::signature::Signer as _;
use p256::ecdsa::{Signature, SigningKey};
use p256::elliptic_curve::sec1::ToEncodedPoint as _;
use p256::{PublicKey, SecretKey};
use rand_core::{OsRng, RngCore as _};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use worker::{Fetch, Headers, Method, Request, RequestInit};

/// How long the push service keeps a message for a device that is offline.
/// A medication reminder older than the 15-minute window is of no use.
const TTL_SECONDS: u32 = 900;
const RECORD_SIZE: u32 = 4096;
const VAPID_LIFETIME_SECONDS: u64 = 12 * 60 * 60;

/// A subscription the browser handed over: where to send, and the keys that
/// encrypt for that browser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PushError {
    /// A key was not valid base64url or not a point on the curve.
    BadKey,
    Encrypt,
}

/// Decodes the key and the secret of a subscription.
fn decode_keys(subscription: &Subscription) -> Result<(PublicKey, Vec<u8>), PushError> {
    let p256dh = B64
        .decode(&subscription.p256dh)
        .map_err(|_| PushError::BadKey)?;
    let public = PublicKey::from_sec1_bytes(&p256dh).map_err(|_| PushError::BadKey)?;
    let auth = B64
        .decode(&subscription.auth)
        .map_err(|_| PushError::BadKey)?;
    if auth.len() != 16 {
        return Err(PushError::BadKey);
    }
    Ok((public, auth))
}

/// Whether the keys of `subscription` are usable.
pub fn check_keys(subscription: &Subscription) -> Result<(), PushError> {
    decode_keys(subscription).map(|_| ())
}

/// The body of a push message for `subscription`, encrypted with the sender's
/// one-time key `sender` and a random `salt`.
pub fn encrypt_with(
    subscription: &Subscription,
    plaintext: &[u8],
    sender: &SecretKey,
    salt: [u8; 16],
) -> Result<Vec<u8>, PushError> {
    let (receiver, auth) = decode_keys(subscription)?;
    let sender_public = sender.public_key().to_encoded_point(false);
    let receiver_point = receiver.to_encoded_point(false);

    let shared = diffie_hellman(sender.to_nonzero_scalar(), receiver.as_affine());
    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(receiver_point.as_bytes());
    key_info.extend_from_slice(sender_public.as_bytes());
    let mut ikm = [0u8; 32];
    Hkdf::<Sha256>::new(Some(&auth), shared.raw_secret_bytes())
        .expand(&key_info, &mut ikm)
        .map_err(|_| PushError::Encrypt)?;

    let hkdf = Hkdf::<Sha256>::new(Some(&salt), &ikm);
    let mut key = [0u8; 16];
    let mut nonce = [0u8; 12];
    hkdf.expand(b"Content-Encoding: aes128gcm\0", &mut key)
        .map_err(|_| PushError::Encrypt)?;
    hkdf.expand(b"Content-Encoding: nonce\0", &mut nonce)
        .map_err(|_| PushError::Encrypt)?;

    // One record: the message, then the 0x02 delimiter that ends the last one.
    let mut record = plaintext.to_vec();
    record.push(2);
    let cipher = Aes128Gcm::new_from_slice(&key).map_err(|_| PushError::Encrypt)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), record.as_slice())
        .map_err(|_| PushError::Encrypt)?;

    let mut body = Vec::with_capacity(86 + ciphertext.len());
    body.extend_from_slice(&salt);
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(u8::try_from(sender_public.len()).map_err(|_| PushError::Encrypt)?);
    body.extend_from_slice(sender_public.as_bytes());
    body.extend_from_slice(&ciphertext);
    Ok(body)
}

/// Encrypts with a fresh key and salt.
pub fn encrypt(subscription: &Subscription, plaintext: &[u8]) -> Result<Vec<u8>, PushError> {
    let sender = SecretKey::random(&mut OsRng);
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    encrypt_with(subscription, plaintext, &sender, salt)
}

/// The origin of a push service URL, which is the `aud` of the VAPID token.
fn audience(endpoint: &str) -> Option<String> {
    let rest = endpoint.strip_prefix("https://")?;
    let host = rest.split('/').next().filter(|host| !host.is_empty())?;
    Some(format!("https://{host}"))
}

/// A VAPID `Authorization` header value: `vapid t=<jwt>, k=<public key>`.
/// `private_key` is the raw 32-byte key in base64url; `now` is Unix seconds.
pub fn vapid_authorization(
    endpoint: &str,
    subject: &str,
    private_key: &str,
    now: u64,
) -> Result<String, PushError> {
    let aud = audience(endpoint).ok_or(PushError::BadKey)?;
    let raw = B64.decode(private_key).map_err(|_| PushError::BadKey)?;
    let signing = SigningKey::from_slice(&raw).map_err(|_| PushError::BadKey)?;
    let header = B64.encode(br#"{"typ":"JWT","alg":"ES256"}"#);
    let claims = B64.encode(
        serde_json::json!({ "aud": aud, "exp": now + VAPID_LIFETIME_SECONDS, "sub": subject })
            .to_string(),
    );
    let signing_input = format!("{header}.{claims}");
    let signature: Signature = signing.sign(signing_input.as_bytes());
    let public = signing.verifying_key().to_encoded_point(false);
    Ok(format!(
        "vapid t={signing_input}.{}, k={}",
        B64.encode(signature.to_bytes()),
        B64.encode(public.as_bytes())
    ))
}

/// What the push service said, grouped by what the sender does next.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The push service accepted the message. The device may not have shown it.
    Accepted,
    /// 404 or 410: the subscription is gone.
    Expired,
    /// 429 or 5xx: try again within the deadline.
    Retry,
    /// Anything else; do not retry.
    Rejected(u16),
}

impl Outcome {
    #[must_use]
    pub fn from_status(status: u16) -> Self {
        match status {
            200..=299 => Self::Accepted,
            404 | 410 => Self::Expired,
            429 | 500..=599 => Self::Retry,
            other => Self::Rejected(other),
        }
    }
}

/// Encrypts `plaintext` for `subscription` and posts it to the push service.
pub async fn send(
    subscription: &Subscription,
    plaintext: &[u8],
    subject: &str,
    private_key: &str,
    now: u64,
) -> worker::Result<Outcome> {
    let body = encrypt(subscription, plaintext)
        .map_err(|error| worker::Error::RustError(format!("push encrypt: {error:?}")))?;
    let authorization = vapid_authorization(&subscription.endpoint, subject, private_key, now)
        .map_err(|error| worker::Error::RustError(format!("vapid: {error:?}")))?;
    let headers = Headers::new();
    headers.set("Authorization", &authorization)?;
    headers.set("Content-Encoding", "aes128gcm")?;
    headers.set("Content-Type", "application/octet-stream")?;
    headers.set("TTL", &TTL_SECONDS.to_string())?;
    headers.set("Urgency", "high")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(js_sys_body(&body)));
    let request = Request::new_with_init(&subscription.endpoint, &init)?;
    let response = Fetch::Request(request).send().await?;
    Ok(Outcome::from_status(response.status_code()))
}

fn js_sys_body(bytes: &[u8]) -> wasm_bindgen::JsValue {
    js_sys::Uint8Array::from(bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 8291, section 5 and Appendix A.
    const AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";
    const UA_PUBLIC: &str =
        "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
    const AS_PRIVATE: &str = "yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw";
    const SALT: &str = "DGv6ra1nlYgDCS1FRnbzlw";
    const EXPECTED: &str = "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPTpK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN";

    fn subscription() -> Subscription {
        Subscription {
            endpoint: "https://push.example.net/push/abc".to_owned(),
            p256dh: UA_PUBLIC.to_owned(),
            auth: AUTH.to_owned(),
        }
    }

    #[test]
    fn matches_the_rfc_8291_example() {
        let sender = SecretKey::from_slice(&B64.decode(AS_PRIVATE).unwrap()).unwrap();
        let salt: [u8; 16] = B64.decode(SALT).unwrap().try_into().unwrap();
        let body = encrypt_with(
            &subscription(),
            b"When I grow up, I want to be a watermelon",
            &sender,
            salt,
        )
        .unwrap();
        assert_eq!(B64.encode(body), EXPECTED);
    }

    #[test]
    fn rejects_malformed_keys() {
        let mut bad = subscription();
        bad.p256dh = "AAAA".to_owned();
        assert_eq!(encrypt(&bad, b"x"), Err(PushError::BadKey));
        let mut bad = subscription();
        bad.auth = "AAAA".to_owned();
        assert_eq!(encrypt(&bad, b"x"), Err(PushError::BadKey));
    }

    #[test]
    fn vapid_token_verifies_with_the_advertised_key() {
        use p256::ecdsa::VerifyingKey;
        use p256::ecdsa::signature::Verifier as _;
        let secret = SecretKey::random(&mut OsRng);
        let private = B64.encode(secret.to_bytes());
        let header = vapid_authorization(
            "https://push.example.net/push/abc",
            "mailto:ops@example.com",
            &private,
            1_000,
        )
        .unwrap();
        let (token, key) = header
            .strip_prefix("vapid t=")
            .and_then(|rest| rest.split_once(", k="))
            .unwrap();
        let (signing_input, signature) = token.rsplit_once('.').unwrap();
        let verifying = VerifyingKey::from_sec1_bytes(&B64.decode(key).unwrap()).unwrap();
        let signature = Signature::from_slice(&B64.decode(signature).unwrap()).unwrap();
        verifying
            .verify(signing_input.as_bytes(), &signature)
            .unwrap();
        let claims = B64
            .decode(signing_input.split('.').nth(1).unwrap())
            .unwrap();
        let claims: serde_json::Value = serde_json::from_slice(&claims).unwrap();
        assert_eq!(claims["aud"], "https://push.example.net");
        assert_eq!(claims["exp"], 1_000 + VAPID_LIFETIME_SECONDS);
        assert_eq!(claims["sub"], "mailto:ops@example.com");
    }

    #[test]
    fn classifies_push_service_statuses() {
        assert_eq!(Outcome::from_status(201), Outcome::Accepted);
        assert_eq!(Outcome::from_status(410), Outcome::Expired);
        assert_eq!(Outcome::from_status(404), Outcome::Expired);
        assert_eq!(Outcome::from_status(429), Outcome::Retry);
        assert_eq!(Outcome::from_status(503), Outcome::Retry);
        assert_eq!(Outcome::from_status(400), Outcome::Rejected(400));
    }
}
