"""Copy the pre-cf local D1 into cf's UUID, leaving the source untouched.

Stop development servers first. PERSIST_DIR defaults to .wrangler/state.
Run before db:migrate:local; an existing destination schema is refused.
"""
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile

from auth_fixture import LOCAL_DATABASE_ID, ROOT

persist = str(Path(os.environ.get("PERSIST_DIR", ROOT / ".wrangler/state")).resolve())
cf = ["npx", "--no-install", "cf", "d1", "raw", LOCAL_DATABASE_ID,
      "--local", "--persist-to", persist]


def run(command):
    # Never echo database contents on an import error.
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    if result.returncode:
        raise SystemExit("Local D1 command failed; source DB is unchanged.")
    return result.stdout


tables = json.loads(run(cf + ["--sql", "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE '_cf_%' AND name NOT LIKE 'sqlite_%'"]))
if tables[0]["results"]["rows"]:
    raise SystemExit("Destination has tables. Refusing to overwrite it; source DB is unchanged.")

with tempfile.TemporaryDirectory(prefix="hypersig-local-import-") as directory:
    dump = Path(directory) / "legacy.sql"
    # Wrangler's local export has no --persist-to option. Give its temporary
    # config the same state directory via a symlink instead.
    config = Path(directory) / "wrangler.toml"
    config.write_text((ROOT / "scripts/legacy-local.toml").read_text())
    (Path(directory) / ".wrangler").mkdir()
    (Path(directory) / ".wrangler/state").symlink_to(persist, target_is_directory=True)
    run(["npx", "--no-install", "wrangler", "d1", "export", "app", "--local",
         "--config", str(config),
         "--output", str(dump)])
    statements = []
    pending = ""
    for line in dump.read_text().splitlines(keepends=True):
        pending += line
        if sqlite3.complete_statement(pending):
            statement = pending.strip()
            if statement.upper() not in ("BEGIN TRANSACTION;", "COMMIT;", "PRAGMA FOREIGN_KEYS=OFF;"):
                statements.append({"sql": statement})
            pending = ""
    if pending.strip():
        raise SystemExit("Incomplete SQL in export; source DB is unchanged.")
    if not statements:
        raise SystemExit("No legacy schema found; source DB is unchanged.")
    batch = Path(directory) / "batch.json"
    batch.write_text(json.dumps([{"sql": "PRAGMA defer_foreign_keys=ON"}, *statements]))
    run(cf + ["--batch", "@" + str(batch)])
print("Copied legacy local D1. Source retained; run npm run db:migrate:local next.")
