"""Smoke test a disposable local Worker whose D1 is seeded with two users.

Run `npm run test:worker` to create an isolated cf dev server on port 8791,
with dummy Google settings and migrations applied to the same PERSIST_DIR.
No Google account is used: sessions are seeded into the local
database by `auth_fixture.py`. Never point it at a deployed Worker.
"""
import json
import os
from pathlib import Path
import re
import sys
import urllib.error
import urllib.request
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parent))
import auth_fixture

PERSIST = os.environ.get('PERSIST_DIR', '/tmp/hypersig-learn-e2e')
base = 'http://localhost:8791'
checks = 0
suffix = uuid.uuid4().hex


def rfc_fixture(name):
    """Reuse the public RFC 8291 fixture already used by Rust crypto tests."""
    source = (Path(__file__).resolve().parents[1] / 'src/push.rs').read_text()
    match = re.search(rf'const {name}: &str\s*=\s*"([^"]+)"', source)
    assert match, name
    return match[1]


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


opener = urllib.request.build_opener(NoRedirect)


NULL = object()


def request(path, data=None, expected=200, cookie=None, headers=None, method=None):
    global checks
    h = {'Accept': 'text/event-stream', 'Sec-Fetch-Site': 'same-origin', 'Content-Type': 'application/json'}
    if cookie:
        h['Cookie'] = cookie
    if headers:
        h.update(headers)
    r = urllib.request.Request(base + path, data=b'null' if data is NULL else json.dumps(data).encode() if data is not None else None, headers=h, method='POST' if data is NULL else method)
    try:
        response = opener.open(r)
    except urllib.error.HTTPError as e:
        response = e
    body = response.read().decode()
    assert response.status == expected, (path, response.status, body)
    checks += 1
    return (response, body)


def page(path, expected, cookie=None):
    return request(path, expected=expected, cookie=cookie, headers={'Accept': 'text/html'})


user_a, user_b = 'e2e-a-' + suffix, 'e2e-b-' + suffix
for user in (user_a, user_b):
    auth_fixture.add_user(PERSIST, user)
cookie_a = auth_fixture.new_session(PERSIST, user_a)
cookie_b = auth_fixture.new_session(PERSIST, user_b)

# Signed-out visitors: pages go to the login page, actions and streams get 401.
request('/medications/save', {}, 401)
request('/clock', expected=401)
(r, _) = page('/medications', 303)
assert r.headers['Location'] == '/login'
page('/login', 200)
# The password sign-in of the earlier design is gone, and its cookie is no session.
request('/auth/login', 'anything', 401)
request('/medications/load', 'x', 401, cookie='__Host-medication=owner:9999999999.x')
# A seeded cookie that names no stored session, and one with a forged id.
request('/medications/load', 'x', 401, cookie='__Host-session=AAAAAAAAAAAAAAAAAAAAAA')
request('/medications/load', 'x', 401, cookie=cookie_a + 'tampered')

# Google sign-in start: a redirect to Google with PKCE and a session cookie for the flow only.
(r, _) = request('/auth/google', expected=303)
location = r.headers['Location']
assert location.startswith('https://accounts.google.com/'), location
assert 'code_challenge_method=S256' in location and 'scope=openid' in location
assert 'client_id=' in location and 'nonce=' in location and 'state=' in location
set_cookie = r.headers['Set-Cookie']
assert all(x in set_cookie for x in ['HttpOnly', 'Secure', 'SameSite=Lax', 'Path=/', '__Host-session=']), set_cookie
assert 'Domain' not in set_cookie and r.headers['Cache-Control'] == 'no-store'
flow_cookie = set_cookie.split(';')[0]
state = re.search(r'state=([^&]+)', location)[1]
request('/medications/load', 'x', 401, cookie=flow_cookie)
# Callbacks: no cookie, a foreign state, Google's refusal, then a replayed state.
for query, cookie in [(f'state={state}&code=x', None), ('state=unknown&code=x', flow_cookie), ('', flow_cookie)]:
    (r, _) = request('/auth/google/callback?' + query, expected=303, cookie=cookie)
    assert r.headers['Location'] == '/login' and r.headers['Cache-Control'] == 'no-store'
(r, _) = request(f'/auth/google/callback?state={state}&error=access_denied', expected=303, cookie=flow_cookie)
assert r.headers['Location'] == '/login'
(r, _) = request(f'/auth/google/callback?state={state}&code=x', expected=303, cookie=flow_cookie)
assert not auth_fixture.query(PERSIST, f"SELECT 1 FROM oauth_flows WHERE state='{state}'")

# A signed-in user. The login page links to the sign-in start route with a plain anchor.
(_, login_html) = page('/login', 200)
assert re.search(r'<a [^>]*href="/auth/google"', login_html), login_html
for path in ['/', '/medications', '/notifications']:
    (_, html) = page(path, 200, cookie_a)
    assert 'ログアウト' in html and '/auth/logout' in html, path
(_, notifications_html) = page('/notifications', 200, cookie_a)
assert user_a not in notifications_html and user_b not in notifications_html and '利用者 ID' not in notifications_html
# Touching a signed-in session on a sign-in route cannot move its deadline.
def deadline(cookie):
    import base64, hashlib
    session_id = cookie.split('=', 1)[1]
    key = base64.urlsafe_b64encode(hashlib.sha256(session_id.encode()).digest()).rstrip(b'=').decode()
    return auth_fixture.query(PERSIST, f"SELECT expires_at FROM sessions WHERE id='{key}'")[0]['expires_at'] // 1000
before = deadline(cookie_a)
(r, _) = request('/auth/google', expected=303, cookie=cookie_a)
assert deadline(cookie_a) == before
request('/auth/google/callback?state=none&code=x', expected=303, cookie=cookie_a)
assert deadline(cookie_a) == before
page('/login', 200, cookie_a)
assert deadline(cookie_a) == before
# The deadline inside the record is what counts: a stored row that outlives it is refused.
stale = auth_fixture.new_session(PERSIST, user_a, lifetime_s=-60, row_lifetime_s=3600)
request('/medications/load', 'x', 401, cookie=stale)
e = {'id': 'runtime-med-' + suffix, 'revision': 0, 'enabled': True, 'medication': {'name': 'test medication', 'start': '2026-10-01', 'end': '', 'repeat': 'daily', 'interval': 1, 'weekdays': 0}, 'slots': [{'id': 'runtime-slot-' + suffix, 'input': {'time': '08:00', 'amount': '0.5', 'unit': 'tablet', 'timing': 'after_meal', 'note': ''}}], 'pauses': []}
(r, body) = request('/medications/save', e, cookie=cookie_a)
assert 'upsert' in body and 'initial' in body and r.headers['Cache-Control'] == 'no-store'
(_, body) = request('/medications/load', 'runtime-med-' + suffix, cookie=cookie_a)
assert 'test medication' in body
request('/medications/save', e, 409, cookie=cookie_a)
e['revision'] = 1
e['enabled'] = False
request('/medications/save', e, cookie=cookie_a)
(_, body) = request('/medications/load', 'runtime-med-' + suffix, cookie=cookie_a)
assert '"revision":2' in body and '"enabled":false' in body
request('/medications/save', e, 403, cookie=cookie_a, headers={'Sec-Fetch-Site': 'cross-site'})
request('/medications/save', e, 415, cookie=cookie_a, headers={'Content-Type': 'text/plain'})
request('/notifications/subscription', {'endpoint': 'https://evil.example/path', 'p256dh': 'a', 'auth': 'a'}, 400, cookie=cookie_a)
sub = {'endpoint': 'https://fcm.googleapis.com/fcm/send/local-test-' + suffix, 'p256dh': rfc_fixture('UA_PUBLIC'), 'auth': rfc_fixture('AUTH')}
request('/notifications/subscription', sub, cookie=cookie_a)
request('/notifications/subscription', sub, cookie=cookie_a)
(_, body) = request('/notifications/subscription/status', sub['endpoint'], cookie=cookie_a)
assert '"saved":true' in body

# The counter is each user's own: a save by one is invisible to the other.
request('/save', 7, cookie=cookie_a)
request('/save', 9, cookie=cookie_b)
counters = {row['owner']: row['count'] for row in auth_fixture.query(PERSIST, f"SELECT owner,count FROM counters WHERE owner IN ('{user_a}','{user_b}')")}
assert counters == {user_a: 7, user_b: 9}, counters
# A user schedules and cancels only a probe of their own subscription.
request('/notifications/probe', sub['endpoint'], 409, cookie=cookie_b)
request('/notifications/probe', NULL, 400, cookie=cookie_a)
other_device = dict(sub, endpoint=sub['endpoint'] + '-other-device')
request('/notifications/subscription', other_device, cookie=cookie_a)
request('/notifications/probe', sub['endpoint'], cookie=cookie_a)
probe = auth_fixture.query(PERSIST, f"SELECT endpoint,state,attempt_count FROM push_probe WHERE owner='{user_a}'")
assert probe == [{'endpoint': sub['endpoint'], 'state': 'pending', 'attempt_count': 0}], probe
request('/notifications/unsubscribe', other_device['endpoint'], cookie=cookie_a)

# Another user cannot read, change, delete, take over or cancel anything of the first.
request('/medications/load', 'runtime-med-' + suffix, 404, cookie=cookie_b)
other = dict(e, revision=2)
request('/medications/save', other, 404, cookie=cookie_b)
request('/medications/remove', ['runtime-med-' + suffix, 2], 409, cookie=cookie_b)
request('/notifications/subscription', sub, 409, cookie=cookie_b)
request('/notifications/unsubscribe', sub['endpoint'], 409, cookie=cookie_b)
(_, body) = request('/notifications/subscription/status', sub['endpoint'], cookie=cookie_b)
assert '"saved":false' in body
(_, body) = request('/notifications/subscription/status', sub['endpoint'], cookie=cookie_a)
assert '"saved":true' in body
request('/notifications/unsubscribe', sub['endpoint'], cookie=cookie_a)
(_, body) = request('/notifications/subscription/status', sub['endpoint'], cookie=cookie_a)
assert '"saved":false' in body
request('/medications/remove', ['runtime-med-' + suffix, 2], cookie=cookie_a)
request('/medications/load', 'runtime-med-' + suffix, 404, cookie=cookie_a)

# Revocation: a version bump and a disabled user end every session at once; expiry ends one.
auth_fixture.execute(PERSIST, f"UPDATE users SET session_version=session_version+1 WHERE id='{user_b}'")
request('/medications/load', 'x', 401, cookie=cookie_b)
auth_fixture.execute(PERSIST, f"UPDATE users SET status='disabled' WHERE id='{user_a}'")
request('/medications/load', 'x', 401, cookie=cookie_a)
auth_fixture.execute(PERSIST, f"UPDATE users SET status='active' WHERE id='{user_a}'")
request('/medications/load', 'x', 404, cookie=cookie_a)
expired = auth_fixture.new_session(PERSIST, user_a, lifetime_s=-60)
request('/medications/load', 'x', 401, cookie=expired)

# Logout deletes the stored record and clears the cookie; the old cookie is then refused.
stored = len(auth_fixture.query(PERSIST, 'SELECT id FROM sessions'))
r = urllib.request.Request(base + '/auth/logout', data=b'null', headers={'Cookie': cookie_a, 'Accept': 'text/event-stream', 'Sec-Fetch-Site': 'same-origin', 'Content-Type': 'application/json'})
with urllib.request.urlopen(r) as response:
    assert response.status == 200 and 'Max-Age=0' in response.headers['Set-Cookie']
checks += 1
assert len(auth_fixture.query(PERSIST, 'SELECT id FROM sessions')) == stored - 1
request('/medications/load', 'x', 401, cookie=cookie_a)
# Unauthenticated starts and callbacks are bounded: a callback without a started flow
# writes no session row, and a client gets a fixed number of live flows.
def count(table):
    return auth_fixture.query(PERSIST, f'SELECT count(*) AS n FROM {table}')[0]['n']
auth_fixture.execute(PERSIST, 'DELETE FROM oauth_flows')
sessions_before = count('sessions')
for query in ['', 'state=x&code=y', 'state=x&error=access_denied']:
    request('/auth/google/callback?' + query, expected=303, cookie='__Host-session=AAAAAAAAAAAAAAAAAAAAAA')
    request('/auth/google/callback?' + query, expected=303)
assert count('sessions') == sessions_before
for _ in range(10):
    request('/auth/google', expected=303)
assert count('oauth_flows') == 10 and count('sessions') <= sessions_before + 10
(r, body) = request('/auth/google', expected=429)
assert r.headers['Retry-After'] == '600' and 'Set-Cookie' not in r.headers and r.headers['Cache-Control'] == 'no-store'
assert count('oauth_flows') == 10 and count('sessions') <= sessions_before + 10
auth_fixture.execute(PERSIST, "UPDATE oauth_flows SET expires_at=1")
request('/auth/google', expected=303)
auth_fixture.execute(PERSIST, 'DELETE FROM oauth_flows')
request('/manifest.webmanifest')
print(f'{checks} local Worker HTTP checks passed')
