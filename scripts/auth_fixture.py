"""Seed users and sessions into a disposable local D1, in place of a Google sign-in.

The Worker keeps a session as a row keyed by the SHA-256 of the session id,
holding the `tower-sessions` record as JSON. The same rows are written here
so a local test can hold a cookie without a Google account. Use it only with
`cf d1 raw --local`; never against a deployed database.
"""
import base64
import datetime
import hashlib
import json
import secrets
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCAL_DATABASE_ID = "00000000-0000-4000-8000-000000000001"
COOKIE = "__Host-session"


def _b64(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def execute(persist_dir, sql):
    """Run `sql` against the local D1 kept in `persist_dir`."""
    subprocess.run(
        ["npx", "--no-install", "cf", "d1", "raw", LOCAL_DATABASE_ID, "--local", "--persist-to", str(persist_dir), "--sql", sql],
        cwd=ROOT, check=True, capture_output=True,
    )


def query(persist_dir, sql):
    out = subprocess.run(
        ["npx", "--no-install", "cf", "d1", "raw", LOCAL_DATABASE_ID, "--local", "--persist-to", str(persist_dir), "--sql", sql],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    result = json.loads(out)[0]["results"]
    return [dict(zip(result["columns"], row)) for row in result["rows"]]


def add_user(persist_dir, user_id, status="active", version=1):
    execute(persist_dir, f"INSERT INTO users(id,status,session_version,created_at) VALUES('{user_id}','{status}',{version},1000)")


def new_session(persist_dir, user_id, version=1, lifetime_s=3600, row_lifetime_s=None):
    """Returns the Cookie header value of a signed-in session for `user_id`.

    `lifetime_s` is the session's own deadline; `row_lifetime_s` is how long the
    stored row lasts, which defaults to the same."""
    number = secrets.randbits(127)
    session_id = _b64(number.to_bytes(16, "little"))
    now = datetime.datetime.now(datetime.timezone.utc)
    expires = now + datetime.timedelta(seconds=lifetime_s)
    row_expires = now + datetime.timedelta(seconds=lifetime_s if row_lifetime_s is None else row_lifetime_s)
    record = {
        "id": number,
        "data": {"identity": {"user_id": user_id, "session_version": version, "level": "Google", "expires_at": int(expires.timestamp())}},
        "expiry_date": [row_expires.year, row_expires.timetuple().tm_yday, row_expires.hour, row_expires.minute, row_expires.second, 0, 0, 0, 0],
    }
    key = _b64(hashlib.sha256(session_id.encode()).digest())
    body = json.dumps(record).replace("'", "''")
    execute(persist_dir, f"INSERT INTO sessions(id,record,expires_at) VALUES('{key}','{body}',{int(row_expires.timestamp() * 1000)})")
    return f"{COOKIE}={session_id}"
