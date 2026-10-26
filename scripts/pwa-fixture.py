"""Create a browser session in the disposable D1 used by test-worker.sh."""
import json
import os
import uuid

import auth_fixture

persist = os.environ["PERSIST_DIR"]
owner = "pwa-" + uuid.uuid4().hex
auth_fixture.add_user(persist, owner)
cookie = auth_fixture.new_session(persist, owner)
print(json.dumps({"cookie": cookie, "owner": owner}))
