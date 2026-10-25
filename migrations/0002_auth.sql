-- Users, external identities, server-side sessions and OAuth flows.
-- Instants are Unix milliseconds. Existing medication data is not touched.
CREATE TABLE users (
 id TEXT PRIMARY KEY, status TEXT NOT NULL CHECK(status IN ('active','disabled')),
 session_version INTEGER NOT NULL DEFAULT 1 CHECK(session_version>=1), created_at INTEGER NOT NULL
);
CREATE TABLE external_identities (
 issuer TEXT NOT NULL CHECK(length(issuer) BETWEEN 1 AND 256), subject TEXT NOT NULL CHECK(length(subject) BETWEEN 1 AND 255),
 user_id TEXT NOT NULL REFERENCES users(id), created_at INTEGER NOT NULL, PRIMARY KEY(issuer,subject)
);
CREATE INDEX identities_user ON external_identities(user_id);
CREATE TABLE sessions (
 id TEXT PRIMARY KEY, record TEXT NOT NULL CHECK(json_valid(record)), expires_at INTEGER NOT NULL
);
CREATE INDEX sessions_expiry ON sessions(expires_at);
CREATE TABLE oauth_flows (
 state TEXT PRIMARY KEY, nonce TEXT NOT NULL, pkce_verifier TEXT NOT NULL, binding TEXT NOT NULL, expires_at INTEGER NOT NULL
);
CREATE INDEX flows_expiry ON oauth_flows(expires_at);
