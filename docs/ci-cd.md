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

VAPID の鍵対と連絡先は GitHub Secrets に置く。`deploy` ジョブが `cloudflare/wrangler-action` の `secrets:` と `vars:` で、デプロイのたびに Worker へ渡す。

| GitHub Secret | Worker 側 | 内容 |
|---|---|---|
| `D1_DATABASE_ID` | — | D1 の ID。`deploy` が `wrangler.toml` の `database_name` の次の行に書き足す |
| `VAPID_PUBLIC_KEY` | var | 公開鍵 |
| `VAPID_PRIVATE_KEY` | secret | 秘密鍵 |
| `VAPID_SUBJECT` | secret | `mailto:` 付きの連絡先 |
| `CLOUDFLARE_API_TOKEN` | — | Workers と D1 の編集権限の Token |
| `CLOUDFLARE_ACCOUNT_ID` | — | アカウント ID |

1. `gh auth login` と `npx wrangler login` を済ませ、`scripts/setup-d1.sh` を実行する。D1 が無ければ作り、その ID を `D1_DATABASE_ID` として登録する。何度実行しても同じ ID を登録する。
2. `scripts/setup-vapid.sh` を実行し、連絡先のメールアドレスを入力する。鍵対を作り、3 つの VAPID の値を GitHub Secrets に登録し、`.dev.vars` にも書く（`wrangler dev` 用。gitignore 済み）。
3. GitHub の Secrets に `CLOUDFLARE_API_TOKEN` と `CLOUDFLARE_ACCOUNT_ID` を登録する。
4. main に merge する。以降は自動でデプロイされ、Worker に VAPID の値が渡る。

GitHub Secrets は値を読み戻せない。秘密鍵はパスワードマネージャーなどに控える。

`VAPID_PRIVATE_KEY` が登録済みのとき、`setup-vapid.sh` は中断する。鍵を変えると既存の Push 購読がすべて無効になるため。連絡先だけ変えるときは `scripts/setup-vapid.sh --subject-only` を使い、次のデプロイで反映される。`--dry-run` は鍵の形式だけを確かめ、何も変更しない。

`deploy` ジョブは `D1_DATABASE_ID` が UUID の形でないとき、または VAPID の GitHub Secret が空のときに失敗する。
