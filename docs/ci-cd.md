# CI/CD

`.github/workflows/ci.yml` が pull request と main への push で動く。main への push では、CI が通ったあとに Cloudflare Workers へデプロイする。

| ジョブ | 内容 | 実行条件 |
|---|---|---|
| `check` | `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` | 常に |
| `wasm` | `cargo check --target wasm32-unknown-unknown` | 常に |
| `deploy` | VAPID の設定を確認し、`wrangler deploy` し、`/manifest.webmanifest` が 200 を返すことを確認する | main への push。`check` と `wasm` の成功後 |

`deploy` は `concurrency: deploy` で直列化し、実行中のデプロイを中断しない。スモークテストが `/` でなく `/manifest.webmanifest` を見るのは、将来ページが未ログインをリダイレクトしても CI が壊れないようにするため。

`.github/dependabot.yml` が Cargo と GitHub Actions の依存を週次で更新する。

## 初回セットアップ

1. 手元で `npx wrangler deploy` を一度実行する。D1 が作られて `wrangler.toml` に `database_id` が書かれるので、コミットする。Worker が無いと `wrangler secret` が失敗する。
2. `scripts/setup-vapid.sh` を実行し、連絡先のメールアドレスを入力する。VAPID 鍵対を作り、次を行う。
   - `VAPID_PRIVATE_KEY` と `VAPID_SUBJECT`（`mailto:` 付き）を Worker secret に登録する
   - 公開鍵を `wrangler.toml` の `VAPID_PUBLIC_KEY` に書く。コミットする
   - 両方の secret を `.dev.vars` に書く（`wrangler dev` 用。gitignore 済み）
3. Cloudflare で API Token（Workers と D1 の編集権限）を作り、GitHub の Secrets に `CLOUDFLARE_API_TOKEN` と `CLOUDFLARE_ACCOUNT_ID` を登録する。
4. main に merge する。以降は自動でデプロイされる。

`VAPID_PRIVATE_KEY` が登録済みのとき、`setup-vapid.sh` は中断する。鍵を変えると既存の Push 購読がすべて無効になるため。連絡先だけ変えるときは `scripts/setup-vapid.sh --subject-only` を使う。`--dry-run` は鍵の形式だけを確かめ、何も変更しない。

`deploy` ジョブは `wrangler.toml` の `VAPID_PUBLIC_KEY` が空のとき、または Worker に `VAPID_PRIVATE_KEY` と `VAPID_SUBJECT` が無いときに失敗する。
