"""Seed users and sessions into a disposable local D1, in place of a Google sign-in.

The Worker keeps a session as a row keyed by the SHA-256 of the session id,
holding the `tower-sessions` record as JSON. The same rows are written here
so a local test can hold a cookie without a Google account. Use it only with
`wrangler d1 execute --local`; never against a deployed database.
"""
import base64
import datetime
import hashlib
import json
import secrets
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COOKIE = "__Host-session"


def _b64(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def execute(persist_dir, sql):
    """Run `sql` against the local D1 kept in `persist_dir`."""
    subprocess.run(
        ["npx", "--yes", "wrangler@4.152.0", "d1", "execute", "app", "--local", "--persist-to", str(persist_dir), "--command", sql],
        cwd=ROOT, check=True, capture_output=True,
    )


def query(persist_dir, sql):
    out = subprocess.run(
        ["npx", "--yes", "wrangler@4.152.0", "d1", "execute", "app", "--local", "--persist-to", str(persist_dir), "--json", "--command", sql],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    return json.loads(out)[0]["results"]


def add_user(persist_dir, user_id, status="active", version=1):
    execute(persist_dir, f"INSERT INTO users(id,status,session_version,created_at) VALUES('{user_id}','{status}',{version},1000)")


def new_session(persist_dir, user_id, version=1, lifetime_s=3600):
    """Returns the Cookie header value of a signed-in session for `user_id`."""
    number = secrets.randbits(127)
    session_id = _b64(number.to_bytes(16, "little"))
    expires = datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(seconds=lifetime_s)
    record = {
        "id": number,
        "data": {"identity": {"user_id": user_id, "session_version": version, "level": "Google"}},
        "expiry_date": [expires.year, expires.timetuple().tm_yday, expires.hour, expires.minute, expires.second, 0, 0, 0, 0],
    }
    key = _b64(hashlib.sha256(session_id.encode()).digest())
    body = json.dumps(record).replace("'", "''")
    execute(persist_dir, f"INSERT INTO sessions(id,record,expires_at) VALUES('{key}','{body}',{int(expires.timestamp() * 1000)})")
    return f"{COOKIE}={session_id}"
