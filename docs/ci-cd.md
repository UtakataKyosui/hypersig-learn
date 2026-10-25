# CI/CD

`.github/workflows/ci.yml` が pull request と main への push で動く。品質チェックと WASM チェックを並列に実行し、main では成功後に `.github/workflows/deploy.yml` を呼び出して Cloudflare Workers へデプロイする。デプロイ用ワークフローは `workflow_call` 専用で、PR からは呼ばない。

| ジョブ | 内容 | 実行条件 |
|---|---|---|
| `changes` | Git の差分から必要なジョブを判定する | Dependabot 作成の PR 以外 |
| `check` | `cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`、`cargo test --locked`、`python3 scripts/test-persistence.py` | Rust に影響する変更 |
| `wasm` | `cargo check --locked --target wasm32-unknown-unknown` | Rust に影響する変更 |
| `worker` | ローカル D1 に migration を適用し、`wrangler dev` の Worker に対して `scripts/test-worker.py`（利用者2人のシード、ログイン開始・callback の不正入力、分離、失効、ログアウト）を実行する。Google のアカウントや Secret は使わない | Rust に影響する変更 |
| `deploy` | VAPID と Google ログインの設定を確認し、`wrangler deploy` し、`/manifest.webmanifest` が 200 を返すことを確認する | main への push で配布に影響する変更。必要なチェックの成功後 |

## 差分実行とキャッシュ

Dependabot が作成した PR は全ジョブを skipped にする。作成者で判定するため、人が再実行してもチェックは省略する。ワークフローの実行記録は残るが、ランナーは起動しない。main へのマージ後は通常どおり検証とデプロイを実行するため、依存更新の互換性エラーはそこで検出される。

`scripts/ci-changes.sh` が PR では merge-base から、push では push 前の SHA から差分を調べる。削除と移動も対象にし、比較元が取得できなければ全チェックを実行する。

- `docs/`、`README.md`、`AGENTS.md`、ライセンス、Dependabot 設定だけの変更は、ビルドとデプロイを省略する。
- `wrangler.toml` だけの変更は Rust チェックを省略し、main でデプロイする。
- その他の変更は全チェックを実行する。`assets/` は Rust に埋め込まれるため検証対象。ワークフローやビルドスクリプトの変更も省略しない。

ワークフロー自体は常に起動し、不要なジョブを skipped にする。パスフィルターでワークフローごと省略して必須チェックが Pending のままになる構成は使わない。PR に追加 push されたら古い CI を中断する。main の実行は中断しない。

`Swatinem/rust-cache` は native、WASM check、Worker release を別のキーで保存する。Rust の版、Cargo の依存、設定がキーに含まれ、通常のソース変更では依存のビルド結果を再利用する。release 用は `cache-all-crates` を有効にし、`cargo install` する `worker-build` と Cargo のツール管理ファイルも保持する。npm はダウンロードキャッシュだけを保存し、Wrangler の版をキーに含める。Wrangler の版を上げるときはキャッシュキーも更新する。

キャッシュヒット時も検証コマンドは実行する。速度の改善量は GitHub 上の初回実行とキャッシュ復元後の実行を比較して確認する。

`deploy` は `concurrency: deploy` で直列化し、実行中のデプロイを中断しない。スモークテストが `/` でなく `/manifest.webmanifest` を見るのは、将来ページが未ログインをリダイレクトしても CI が壊れないようにするため。

`deploy` は Wrangler の版を `wranglerVersion` で固定する。`cloudflare/wrangler-action@v3` の既定は Wrangler 3 で、`wrangler.toml` の `[exports.Hub]` を読めないため。Dependabot はこの値を更新しないので、上げるときは手で書き換える。

手元で `npx wrangler deploy` を実行しない。`database_id` と `VAPID_PUBLIC_KEY` を持たない設定で、CI が設定した Worker を上書きしてしまう。

`.github/dependabot.yml` が Cargo と GitHub Actions の依存を週次で更新する。

## 初回セットアップ

VAPID の鍵対と連絡先は GitHub Secrets に置く。`deploy` ジョブが `cloudflare/wrangler-action` の `secrets:` と `vars:` で、デプロイのたびに Worker へ渡す。

| GitHub Secret | Worker 側 | 内容 |
|---|---|---|
| `D1_DATABASE_ID` | — | D1 の ID。`deploy` が `wrangler.toml` の `database_name` の次の行に書き足す |
| `VAPID_PUBLIC_KEY` | var | 公開鍵 |
| `VAPID_PRIVATE_KEY` | secret | 秘密鍵 |
| `VAPID_SUBJECT` | secret | `mailto:` 付きの連絡先 |
| `GOOGLE_CLIENT_ID` | var | Google の OAuth クライアント ID |
| `GOOGLE_CLIENT_SECRET` | secret | Google の OAuth クライアントのシークレット |
| `GOOGLE_REDIRECT_URL` | var | `https://<本番ホスト>/auth/google/callback`。Google Cloud に登録した URI と一致させる |
| `CLOUDFLARE_API_TOKEN` | — | Workers と D1 の編集権限の Token |
| `CLOUDFLARE_ACCOUNT_ID` | — | アカウント ID |

1. `gh auth login` と `npx wrangler login` を済ませ、`scripts/setup-d1.sh` を実行する。D1 が無ければ作り、その ID を `D1_DATABASE_ID` として登録する。何度実行しても同じ ID を登録する。
2. `scripts/setup-vapid.sh` を実行し、連絡先のメールアドレスを入力する。鍵対を作り、3 つの VAPID の値を GitHub Secrets に登録し、`.dev.vars` にも書く（`wrangler dev` 用。gitignore 済み）。
3. [google-auth.md](google-auth.md) に従って Google の OAuth クライアントを作り、`GOOGLE_CLIENT_ID`・`GOOGLE_CLIENT_SECRET`・`GOOGLE_REDIRECT_URL` を GitHub Secrets に登録する。
4. GitHub の Secrets に `CLOUDFLARE_API_TOKEN` と `CLOUDFLARE_ACCOUNT_ID` を登録する。
5. main に merge する。以降は自動でデプロイされ、Worker に VAPID の値が渡る。

GitHub Secrets は値を読み戻せない。秘密鍵はパスワードマネージャーなどに控える。

`VAPID_PRIVATE_KEY` が登録済みのとき、`setup-vapid.sh` は中断する。鍵を変えると既存の Push 購読がすべて無効になるため。連絡先だけ変えるときは `scripts/setup-vapid.sh --subject-only` を使い、次のデプロイで反映される。`--dry-run` は鍵の形式だけを確かめ、何も変更しない。

`deploy` ジョブは `D1_DATABASE_ID` が UUID の形でないとき、VAPID または Google の GitHub Secret が空のとき、`GOOGLE_REDIRECT_URL` が `https://…/auth/google/callback` の形でないときに失敗する。
