# CI/CD

開発・D1・配布は固定した `cf` CLI を使う。設定変換、ローカル DB のコピーと未検証事項は [移行記録（#47）](cloudflare-cli-evaluation.md) を参照。

`.github/workflows/ci.yml` が pull request と main への push で動く。品質チェックと WASM チェックを並列に実行し、main では成功後に `.github/workflows/deploy.yml` を呼び出して Cloudflare Workers へデプロイする。デプロイ用ワークフローは `workflow_call` 専用で、PR からは呼ばない。

| ジョブ | 内容 | 実行条件 |
|---|---|---|
| `changes` | Git の差分から必要なジョブを判定する | Dependabot 作成の PR 以外 |
| `check` | `cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`、`cargo test --locked`、`python3 scripts/test-persistence.py` | Rust に影響する変更 |
| `wasm` | `cargo check --locked --target wasm32-unknown-unknown` | Rust に影響する変更 |
| `worker` | `npm run test:worker` で build と deploy dry-run を確認し、一時ディレクトリの D1 に migration を適用、`cf dev` の Worker に対して `scripts/test-worker.py`（利用者2人のシード、ログイン開始・callback の不正入力、分離、失効、ログアウト）を実行する。Google のアカウントや Secret は使わず、Local Explorer API の cron 呼び出しも確認する | Rust に影響する変更 |
| `deploy` | VAPID と Google ログインの設定を確認し、`cf build --mode production` と dry-run 後、同じ Build Output を `cf deploy --prebuilt --mode production --no-provision` で配布し、`/manifest.webmanifest` が 200 を返すことを確認する | main への push で配布に影響する変更。必要なチェックの成功後 |

## 差分実行とキャッシュ

Dependabot が作成した PR は全ジョブを skipped にする。作成者で判定するため、人が再実行してもチェックは省略する。ワークフローの実行記録は残るが、ランナーは起動しない。main へのマージ後は通常どおり検証とデプロイを実行するため、依存更新の互換性エラーはそこで検出される。

`scripts/ci-changes.sh` が PR では merge-base から、push では push 前の SHA から差分を調べる。削除と移動も対象にし、比較元が取得できなければ全チェックを実行する。

- `docs/`、`README.md`、`AGENTS.md`、ライセンス、Dependabot 設定だけの変更は、ビルドとデプロイを省略する。
- `cloudflare.config.ts`、`wrangler.config.ts`、npm の manifest/lockfile は Worker の起動とビルドに影響するため全チェックと main でのデプロイを実行する。
- その他の変更は全チェックを実行する。`assets/` は Rust に埋め込まれるため検証対象。ワークフローやビルドスクリプトの変更も省略しない。

ワークフロー自体は常に起動し、不要なジョブを skipped にする。パスフィルターでワークフローごと省略して必須チェックが Pending のままになる構成は使わない。PR に追加 push されたら古い CI を中断する。main の実行は中断しない。

`Swatinem/rust-cache` は native、WASM check、Worker release を別のキーで保存する。Rust の版、Cargo の依存、設定がキーに含まれ、通常のソース変更では依存のビルド結果を再利用する。release 用は `cache-all-crates` を有効にし、`cargo install` する `worker-build` と Cargo のツール管理ファイルも保持する。npm のダウンロードキャッシュは `actions/setup-node` が `package-lock.json` をキーに保存し、毎回 `npm ci` を実行する。

キャッシュヒット時も検証コマンドは実行する。速度の改善量は GitHub 上の初回実行とキャッシュ復元後の実行を比較して確認する。

`deploy` は `concurrency: deploy` で直列化し、実行中のデプロイを中断しない。スモークテストが `/` でなく `/manifest.webmanifest` を見るのは、将来ページが未ログインをリダイレクトしても CI が壊れないようにするため。

`package.json` と lockfile が `cf` と Wrangler bundler の版を固定する。Node.js 24 上で `cf` を直接実行し、`cloudflare/wrangler-action` は使わない。本番の binding は `cloudflare.config.ts` から作り、TOML を書き換える工程はない。

手元でデプロイしない。CI が `--mode production`、既存の `D1_DATABASE_ID` と vars/secrets を揃えて配布する。本番設定は必須の値が無ければビルド時に失敗し、`--no-provision` で別 DB の自動作成を防ぐ。smoke test のホストは `GOOGLE_REDIRECT_URL` の origin から取得する。

`.github/dependabot.yml` が Cargo、npm と GitHub Actions の依存を週次で更新する。

## 初回セットアップ

VAPID の鍵対と連絡先は GitHub Secrets に置く。`deploy` ジョブが vars を `bindings.text(...)` へ、secrets を権限 0600 の一時 JSON と `--secrets-file` で渡す。一時ファイルは終了時に削除する。

| GitHub Secret | Worker 側 | 内容 |
|---|---|---|
| `D1_DATABASE_ID` | — | 既存 D1 の UUID。本番の DB binding と migrations に同じ値を渡す |
| `VAPID_PUBLIC_KEY` | var | 公開鍵 |
| `VAPID_PRIVATE_KEY` | secret | 秘密鍵 |
| `VAPID_SUBJECT` | secret | `mailto:` 付きの連絡先 |
| `GOOGLE_CLIENT_ID` | var | Google の OAuth クライアント ID |
| `GOOGLE_CLIENT_SECRET` | secret | Google の OAuth クライアントのシークレット |
| `GOOGLE_REDIRECT_URL` | var | `https://<本番ホスト>/auth/google/callback`。Google Cloud に登録した URI と一致させる |
| `CLOUDFLARE_API_TOKEN` | — | Workers と D1 の編集権限の Token |
| `CLOUDFLARE_ACCOUNT_ID` | — | アカウント ID |

1. `npm ci` の後、`gh auth login` と `npx cf auth login` を済ませ、`scripts/setup-d1.sh` を実行する。D1 が無ければ作り、その ID を `D1_DATABASE_ID` として登録する。何度実行しても同じ ID を登録する。
2. `scripts/setup-vapid.sh` を実行し、連絡先のメールアドレスを入力する。鍵対を作り、3 つの VAPID の値を GitHub Secrets に登録し、`.dev.vars` にも書く（`cf dev` 用。gitignore 済み）。
3. [google-auth.md](google-auth.md) に従って Google の OAuth クライアントを作り、`GOOGLE_CLIENT_ID`・`GOOGLE_CLIENT_SECRET`・`GOOGLE_REDIRECT_URL` を GitHub Secrets に登録する。
4. GitHub の Secrets に `CLOUDFLARE_API_TOKEN` と `CLOUDFLARE_ACCOUNT_ID` を登録する。
5. main に merge する。以降は自動でデプロイされ、Worker に VAPID の値が渡る。

GitHub Secrets は値を読み戻せない。秘密鍵はパスワードマネージャーなどに控える。

`VAPID_PRIVATE_KEY` が登録済みのとき、`setup-vapid.sh` は中断する。鍵を変えると既存の Push 購読がすべて無効になるため。連絡先だけ変えるときは `scripts/setup-vapid.sh --subject-only` を使い、次のデプロイで反映される。`--dry-run` は鍵の形式だけを確かめ、何も変更しない。

`deploy` ジョブは `D1_DATABASE_ID` が UUID の形でないとき、VAPID または Google の GitHub Secret が空のとき、`GOOGLE_REDIRECT_URL` が `https://…/auth/google/callback` の形でないときに失敗する。

## API Tokenの権限と403からの復旧（#48）

Cloudflare Dashboard のプロフィール → API Tokens → Create Token で **Edit Cloudflare Workers** テンプレートを選び、対象アカウントを `CLOUDFLARE_ACCOUNT_ID` のアカウントに限定する。この構成で確認する権限は次のとおり。テンプレートだけでD1権限が付くとは限らない。

| スコープ | 権限 | 用途 |
|---|---|---|
| Account | Workers Scripts: Edit | Workerの配布・設定・Cron schedulesの取得／更新 |
| Account | D1: Edit | CIのmigration適用とD1操作 |
| Account | Account Settings: Read | CLIのアカウント情報取得 |

独自ドメイン／zone routeを追加する場合は、そのzoneのWorkers Routes権限も別途確認する。現在のworkers.dev構成に必要な権限とは分ける。最新の名称は [API token permissions](https://developers.cloudflare.com/fundamentals/api/reference/permissions/)、テンプレートの作成手順は [GitHub Actions公式手順](https://developers.cloudflare.com/workers/ci-cd/external-cicd/github-actions/)、schedulesを含む権限は [Workers権限](https://developers.cloudflare.com/workers/authorization/workers/)を参照する。実トークンの権限内訳はこの作業では読み取っていない。

#48の記録では `Unable to fetch bindings, routes, or services metadata from the dashboard` の背後で `GET /accounts/*/workers/scripts/app/schedules` が403となり、トークン差し替え後に成功した。この汎用メッセージだけで一時障害と決めず、HTTP status・対象account・権限・有効期限を確認する。403以外ならその応答に従って切り分ける。

1. 失敗したActions runとSHAを記録し、失敗ステップのログを確認する。過去runの `Show the Wrangler log` があればその出力も見る。現行cf版workflowにはそのステップがないため、エラーに表示されたWranglerのログパスを確認する。
2. 本文が不足する場合だけ、一時的に `WRANGLER_LOG_SANITIZE=false` を使って管理された環境で調査する。非サニタイズログには秘密が含まれうる。生ログを公開artifactやIssueに貼らず、共有するのは値を除いたstatusとAPIパスだけにする。調査後は設定を戻す。
3. 管理者が権限を修正したトークンを作り、`gh secret set CLOUDFLARE_API_TOKEN` の対話入力で差し替える。値をコマンド引数や履歴へ書かない。
4. `gh run rerun <run-id> --failed` で再実行し、migration・deploy・smoke testの結果と配布SHAを照合する。失敗jobの再実行でも本番DB更新・配布が起きるため、運用担当者が実施する。

トークン差し替え・再実行は今回未実施。成功後は [本番Cron・実機の記録票](mvp-device-runbook.md)へ引き継ぐ。
