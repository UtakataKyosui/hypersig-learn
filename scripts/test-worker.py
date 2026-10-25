"""Smoke test a disposable local Worker with the documented dummy auth settings."""
import json
from pathlib import Path
import re
import urllib.error
import urllib.request
import uuid
base = 'http://localhost:8791'
cookie = None
checks = 0
suffix = uuid.uuid4().hex


def rfc_fixture(name):
    """Reuse the public RFC 8291 fixture already used by Rust crypto tests."""
    source = (Path(__file__).resolve().parents[1] / 'src/push.rs').read_text()
    match = re.search(rf'const {name}: &str\s*=\s*"([^"]+)"', source)
    assert match, name
    return match[1]

def request(path, data=None, expected=200, auth=True, headers=None):
    global checks, cookie
    h = {'Accept': 'text/event-stream', 'Sec-Fetch-Site': 'same-origin', 'Content-Type': 'application/json'}
    if auth and cookie:
        h['Cookie'] = cookie
    if headers:
        h.update(headers)
    r = urllib.request.Request(base + path, data=json.dumps(data).encode() if data is not None else None, headers=h)
    try:
        response = urllib.request.urlopen(r)
    except urllib.error.HTTPError as e:
        response = e
    body = response.read().decode()
    assert response.status == expected, (path, response.status, body)
    checks += 1
    return (response, body)
request('/medications/save', {}, 401, False)
request('/clock', expected=401, auth=False)
(r, _) = request('/auth/login', 'wrong', 401)
(r, body) = request('/auth/login', 'local-test-password')
cookie = r.headers['Set-Cookie'].split(';')[0]
assert all((x in r.headers['Set-Cookie'] for x in ['HttpOnly', 'Secure', 'SameSite=Lax', 'Max-Age=86400']))
request('/medications', headers={'Accept': 'text/html'})
e = {'id': 'runtime-med-' + suffix, 'revision': 0, 'enabled': True, 'medication': {'name': 'test medication', 'start': '2026-10-01', 'end': '', 'repeat': 'daily', 'interval': 1, 'weekdays': 0}, 'slots': [{'id': 'runtime-slot-' + suffix, 'input': {'time': '08:00', 'amount': '0.5', 'unit': 'tablet', 'timing': 'after_meal', 'note': ''}}], 'pauses': []}
(r, body) = request('/medications/save', e)
assert 'upsert' in body and 'initial' in body
(_, body) = request('/medications/load', 'runtime-med-' + suffix)
assert 'test medication' in body
request('/medications/save', e, 409)
e['revision'] = 1
e['enabled'] = False
request('/medications/save', e)
(_, body) = request('/medications/load', 'runtime-med-' + suffix)
assert '"revision":2' in body and '"enabled":false' in body
request('/medications/save', e, 403, headers={'Sec-Fetch-Site': 'cross-site'})
request('/medications/save', e, 415, headers={'Content-Type': 'text/plain'})
e['id'] = 'runtime-other-' + suffix
e['revision'] = 0
request('/medications/save', e, 409)
request('/medications/load', 'runtime-other-' + suffix, 404)
request('/notifications/subscription', {'endpoint': 'https://evil.example/path', 'p256dh': 'a', 'auth': 'a'}, 400)
sub = {'endpoint': 'https://fcm.googleapis.com/fcm/send/local-test-' + suffix, 'p256dh': rfc_fixture('UA_PUBLIC'), 'auth': rfc_fixture('AUTH')}
request('/notifications/subscription', sub)
request('/notifications/subscription', sub)
(_, body) = request('/notifications/subscription/status', sub['endpoint'])
assert '"saved":true' in body
request('/notifications/unsubscribe', sub['endpoint'])
(_, body) = request('/notifications/subscription/status', sub['endpoint'])
assert '"saved":false' in body
request('/medications/delete', {'id': 'runtime-med-' + suffix, 'revision': 2})
request('/medications/load', 'runtime-med-' + suffix, 404)
r = urllib.request.Request(base + '/auth/logout', data=b'null', headers={'Cookie': cookie, 'Accept': 'text/event-stream', 'Sec-Fetch-Site': 'same-origin', 'Content-Type': 'application/json'})
with urllib.request.urlopen(r) as response:
    assert response.status == 200 and 'Max-Age=0' in response.headers['Set-Cookie']
checks += 1
cookie = cookie + 'tampered'
request('/medications/load', 'runtime-med-' + suffix, 401)
request('/manifest.webmanifest', auth=False)
print(f'{checks} local Worker HTTP checks passed')
