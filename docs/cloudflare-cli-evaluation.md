# Cloudflare CLI `cf` への移行

[Issue #47](https://github.com/UtakataKyosui/hypersig-learn/issues/47) の対応記録。2026-10-26、利用者の指示に従い `cf` へ移行した。

## 採用構成

`cf migrate --dry-run --no-install` の後、既存の差分を保持して `cf migrate --no-install --force` を実行した。生成された DO の手動対応を解消し、Worker 名 `app`、`Hub` の SQLite export、`HUB` binding、毎分の cron、compatibility date、Rust のビルドを維持した。[公式の移行手順](https://developers.cloudflare.com/cf/wrangler/migrate/)

| 項目 | 移行後 |
|---|---|
| CLI | `cf@1.0.0-beta.21`、`wrangler@4.152.0` を開発依存として固定。Node.js 22.18 以降、CI は 24 |
| Worker 設定 | `cloudflare.config.ts`。元の `wrangler.toml` は通常の開発・配布では使わない |
| Rust ビルド | `wrangler.config.ts` のカスタムビルドが `scripts/build.sh` を実行。WASM と shim を cf の Build Output に含める |
| 開発 | `npm run dev` → `cf dev`、localhost:8787。保存先は引き続き `.wrangler/state/` |
| ローカル D1 | 開発専用 UUID `00000000-0000-4000-8000-000000000001` を binding と CLI で共用 |
| 本番 D1 | `--mode production` で既存の `D1_DATABASE_ID` を必須にする。新 DB を自動作成しない |
| D1 操作 | `cf d1 list/create/get/raw`、`cf d1 migrations apply <UUID>`。リモートが既定なのでローカルには必ず `--local` |
| 秘密値 | ローカルは従来の `.dev.vars`。本番は GitHub Secrets と一時 JSON の `--secrets-file`。秘密値を設定ファイルへコピーしない |
| CI | 専用 Action を使わず、`npm ci` と `npx --no-install cf` を実行。詳細は [CI/CD](ci-cd.md) |
| 本番ログ | `cf` のライブログは未対応のため `npx wrangler tail app` を継続 |

設定形式は [Programmatic configuration](https://developers.cloudflare.com/cf/projects/cloudflare-config/)、機能差は [Wrangler to cf reference](https://developers.cloudflare.com/cf/wrangler/reference/) を参照。Wrangler は cf のビルド・開発実装として残る。Vite への置換は行わない。

## ローカル D1 の引き継ぎ

従来の設定は UUID を持たず、ローカルの保存キーは binding 名 `DB` だった。`cf d1 migrations apply` はローカルでも UUID を要求するため、保存先のディレクトリだけ合わせても同じ DB にはならない。既存 DB を使っていた場合は、**開発サーバーを止めて、初回 migration より先に**コピーする。

```sh
npm ci
npm run db:import:local
npm run db:migrate:local
npm run dev
```

新規の環境では import を省略する。`scripts/import-local-d1.py` は `scripts/legacy-local.toml` を使って旧 DB を Wrangler でローカル export し、cf で開発用 UUID にまとめて取り込む。旧 DB は残し、コピー先にテーブルがあれば上書きを拒否する。既定の保存先は `.wrangler/state`。別の保存先からコピーする場合は `PERSIST_DIR` にその絶対パスを指定する。

コピーは明示的に実行する。今回、開発者の既存データ自体は操作していない。使い捨てデータで、行・migration 履歴・trigger の保持、元 DB の保持、上書き拒否を検証した。旧設定ファイルはこの export 専用であり、開発・配布には使わない。

## cron と Worker の検証

固定した Wrangler の cf delegate は `--mode`、`--host`、`--port`、`--local` だけを受け付け、`--test-scheduled` や `--var`、`--persist-to` を転送できない。開発時の cron 呼び出しには Local Explorer を使う。

```sh
curl --fail -X POST \\
  'http://localhost:8787/cdn-cgi/local/explorer/api/local/scheduled?worker=app' \\
  -H 'Content-Type: application/json' --data '{"cron":"* * * * *"}'
```

GUI は `/cdn-cgi/local/explorer`。これらは実際にローカルの `scheduled` を呼ぶため、登録済みの通知があれば送信処理も走る。ローカル cron は自動発火しない。[Local Explorer](https://developers.cloudflare.com/workers/local-development/local-explorer/#test-cron-triggers)

`npm run test:worker` は build → deploy dry-run → 一時ディレクトリへの設定コピー → ダミーの `.dev.vars.test` → 同じ保存先への migrations と `cf dev --mode test --port 8791` → HTTP 検証74件 → cron API 呼び出しを行う。通常の D1 と秘密値は使わず、終了時にサーバーと一時ファイルを片付ける。

## CI と本番への反映

CI の配布は本番 mode でビルドし、その Build Output を dry-run と実配布で共用する。DB の ID と公開値3個はビルド時に渡し、秘密値3個は配布時だけ権限 0600 の一時 JSON から渡して削除する。`--no-provision` で既存 DB を維持し、smoke test は既存の `GOOGLE_REDIRECT_URL` の origin に対して行う。[公式の CI 構成](https://developers.cloudflare.com/cf/ci/)

GitHub Secrets の名前は変えない。`cf` のローカル認証は `npx cf auth login` で、Wrangler のログインを再利用しない。CI は従来どおり `CLOUDFLARE_API_TOKEN` と `CLOUDFLARE_ACCOUNT_ID` を使う。[Wrangler ユーザー向けガイド](https://developers.cloudflare.com/cf/wrangler/)

本番デプロイ、実在する DB/DO の継続利用、実機 Push 表示、GitHub 上での CI 成功は、このローカル検証では確認していない。

## 実施した検証

- `cargo check`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（59件）、`cargo check --target wasm32-unknown-unknown`
- `python3 scripts/test-persistence.py`（16件）、`python3 scripts/test-cli.py`（4件）
- `npm run test:worker`（HTTP 74件、cron 呼び出し、build と deploy dry-run）
- ダミーの本番 ID・公開値を使う `cf build --mode production` と `cf deploy --prebuilt --mode production --dry-run --no-provision`
- `actionlint`、変更したシェルスクリプトの `shellcheck`、npm lockfile の dry-run 確認

WASM チェックでは `scripts/build.sh` と同様、rustup の `~/.cargo/bin` を PATH の先頭に置く。別途導入したシステムの Rust には、このターゲットがないことがある。
