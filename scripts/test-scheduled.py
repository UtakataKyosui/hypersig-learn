"""Exercise real cron -> domain -> D1 -> failed sender without external Push.

Only the disposable Worker with deliberately invalid VAPID credentials is used.
The clock-relative fixtures avoid sleeps and never require a browser/SSE.
"""
import datetime as dt
import json
import os
import urllib.request
import uuid

import auth_fixture

persist = os.environ["PERSIST_DIR"]
owner = "cron-" + uuid.uuid4().hex
now = dt.datetime.now(dt.timezone.utc)
jst = now.astimezone(dt.timezone(dt.timedelta(hours=9)))
scheduled = jst.replace(second=0, microsecond=0) - dt.timedelta(minutes=1)
at = int(scheduled.timestamp() * 1000)
date = scheduled.date().isoformat()
auth_fixture.add_user(persist, owner)


def literal(value):
    return "'" + str(value).replace("'", "''") + "'"


for kind in ["due", "paused", "disabled", "new", "ended", "expired"]:
    med = owner + "-" + kind
    slot = med + "-slot"
    instant = scheduled if kind != "expired" else scheduled - dt.timedelta(minutes=30)
    day = instant.date().isoformat()
    time = instant.strftime("%H:%M")
    end = (instant.date() - dt.timedelta(days=1)).isoformat() if kind == "ended" else ""
    start = (instant.date() - dt.timedelta(days=2)).isoformat()
    pauses = [{"id": med + "-pause", "input": {"start": day, "end": day}}] if kind == "paused" else []
    editor = {"id": med, "revision": 1, "enabled": kind != "disabled",
              "medication": {"name": "fixture", "start": start, "end": end, "repeat": "daily", "interval": 1, "weekdays": 0},
              "slots": [{"id": slot, "input": {"time": time, "amount": "1", "unit": "tablet", "timing": "unspecified", "note": ""}}], "pauses": pauses}
    saved = at + 1 if kind == "new" else at - 86_400_000
    auth_fixture.execute(persist, f"""
      INSERT INTO medications VALUES({literal(med)},{literal(owner)},'fixture',{int(editor['enabled'])},1,{saved},{saved},{literal(json.dumps(editor))});
      INSERT INTO schedules VALUES({literal(med)},{literal(med)},{literal(start)},{literal(end) if end else 'NULL'},'daily',1,0,'Asia/Tokyo');
      INSERT INTO dose_slots VALUES({literal(slot)},{literal(med)},{literal(time)},100,'tablet','unspecified','');
    """)

# Keys and VAPID settings are invalid: encryption fails before any network call.
auth_fixture.execute(persist, f"INSERT INTO push_subscriptions VALUES({literal(owner)},{literal(owner)},'https://fcm.googleapis.com/{owner}','invalid','invalid',1,{at-86400000})")
auth_fixture.execute(persist, f"INSERT INTO push_probe(owner,endpoint,p256dh,auth,probe_at,next_attempt_at) VALUES({literal(owner)},'https://fcm.googleapis.com/{owner}','invalid','invalid',{at},{at})")
for _ in range(2):
    request = urllib.request.Request(
        "http://localhost:8791/cdn-cgi/local/explorer/api/local/scheduled?worker=app",
        data=b'{"cron":"* * * * *"}', headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request) as response:
        assert json.load(response)["success"] is True
jobs = auth_fixture.query(persist, f"SELECT j.* FROM notification_jobs j JOIN medications m ON m.id=j.medication_id WHERE m.owner={literal(owner)}")
assert len(jobs) == 1 and jobs[0]["medication_id"] == owner + "-due", jobs
deliveries = auth_fixture.query(persist, f"SELECT * FROM notification_deliveries WHERE subscription_id={literal(owner)}")
assert len(deliveries) == 1, deliveries
assert deliveries[0]["state"] == "pending" and deliveries[0]["attempt_count"] == 1, deliveries
assert deliveries[0]["last_error_class"] == "retry", deliveries
probes = auth_fixture.query(persist, f"SELECT state,attempt_count,last_error_class FROM push_probe WHERE owner={literal(owner)}")
assert probes == [{"state": "pending", "attempt_count": 1, "last_error_class": "retry"}], probes
print("Scheduled domain/D1 integration: duplicate cron, dose/test retry backoff, disabled/pause/save/end/expiry cutoffs OK")
