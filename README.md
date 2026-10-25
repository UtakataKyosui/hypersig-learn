# hypersig-learn

[hypersig](https://imaimai17468.github.io/hypersig/) 0.4.1 で作る Web アプリ。Rust を `wasm32-unknown-unknown` にコンパイルして Cloudflare Workers で動かし、データは D1、Web Push の配信は Cron で行う。

## 必要なもの

| ツール | 用途 |
|---|---|
| [rustup](https://rustup.rs/) | `rust-toolchain.toml` が stable と wasm32 ターゲットを指定する |
| Node.js 22.18 以降（`npm`） | 固定した `cf` と Wrangler bundler の実行 |
| [GitHub CLI](https://cli.github.com/) | セットアップスクリプトが GitHub Secrets を登録する |
| `openssl` | VAPID 鍵の生成 |

## ローカルで動かす

```sh
npm ci
# 初回。従来のローカル DB を残したい場合は、この前に npm run db:import:local
npm run db:migrate:local
npm run dev
```

http://localhost:8787 で開く。D1 は `.wrangler/state/` の SQLite に保存される。従来の DB からのコピー手順と CLI の制約は [cf 移行記録](docs/cloudflare-cli-evaluation.md) を参照。Web Push を試すには、次の「セットアップ」の手順 2 で `.dev.vars` に VAPID の値を書く。

開発用 D1 は固定 UUID、本番 D1 は GitHub Secret `D1_DATABASE_ID` で選ぶ。
`.dev.vars` に `GOOGLE_CLIENT_ID`、`GOOGLE_CLIENT_SECRET`、`GOOGLE_REDIRECT_URL` を設定し、`/login` から Google でログインする。
本番では同名の GitHub Secrets が必要。Google の設定、セッション、失効、検証結果は [docs/google-auth.md](docs/google-auth.md)、利用者ごとのデータ分離と既存データの引き継ぎは [docs/data-isolation.md](docs/data-isolation.md)、保存契約は [docs/persistence-auth.md](docs/persistence-auth.md) を参照。

## セットアップ（初回のみ）

デプロイは GitHub Actions が行う。必要な設定は GitHub Secrets に置き、デプロイのたびに Worker へ渡される。

1. `gh auth login` と `npx cf auth login` を済ませる。
2. `scripts/setup-d1.sh` を実行する。D1 が無ければ作り、ID を `D1_DATABASE_ID` として登録する。
3. `scripts/setup-vapid.sh` を実行し、連絡先のメールアドレスを入力する。VAPID の鍵対を作り、`VAPID_PUBLIC_KEY`、`VAPID_PRIVATE_KEY`、`VAPID_SUBJECT` を登録し、`.dev.vars` にも書く。**秘密鍵は GitHub から読み戻せない。** パスワードマネージャーなどに控える。
4. Cloudflare で Workers と D1 の編集権限を持つ API Token を作り、GitHub Secrets に `CLOUDFLARE_API_TOKEN` と `CLOUDFLARE_ACCOUNT_ID` を登録する。
5. main に merge する。

鍵を作り直すと既存の Push 購読がすべて無効になるため、`VAPID_PRIVATE_KEY` が登録済みのとき `setup-vapid.sh` は中断する。連絡先だけ変えるときは `scripts/setup-vapid.sh --subject-only` を使う。

## 開発の流れ

変更ごとに次を通す。

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
python3 scripts/test-persistence.py
cargo check --target wasm32-unknown-unknown
npm run test:worker
```

pull request では `check`（fmt、clippy、test）と `wasm` が動く。main への push では、これらが通ったあとに Cloudflare Workers へデプロイし、`/manifest.webmanifest` が 200 を返すことを確かめる。

## ドキュメント

| ページ | 内容 |
|---|---|
| [docs/ci-cd.md](docs/ci-cd.md) | CI/CD の構成、GitHub Secrets の一覧、セットアップの詳細 |
| [docs/medication-ui.md](docs/medication-ui.md) | 服薬設定画面の構成、状態表示、差分更新、複数タブ同期の採否、検証状況（#5） |
| [docs/cloudflare-cli-evaluation.md](docs/cloudflare-cli-evaluation.md) | `cf` CLI への移行記録、ローカル DB の引き継ぎと制約（#47） |
| [docs/mvp0-web-push-verification.md](docs/mvp0-web-push-verification.md) | Web Push の検証記録と実機での手順 |
