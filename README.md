# hypersig-learn

[hypersig](https://imaimai17468.github.io/hypersig/) 0.4.1 で作る Web アプリ。Rust を `wasm32-unknown-unknown` にコンパイルして Cloudflare Workers で動かし、データは D1、Web Push の配信は Cron で行う。

## 必要なもの

| ツール | 用途 |
|---|---|
| [rustup](https://rustup.rs/) | `rust-toolchain.toml` が stable と wasm32 ターゲットを指定する |
| Node.js（`npx`） | `wrangler` の実行 |
| [GitHub CLI](https://cli.github.com/) | セットアップスクリプトが GitHub Secrets を登録する |
| `openssl` | VAPID 鍵の生成 |

## ローカルで動かす

```sh
npx wrangler dev
```

http://localhost:8787 で開く。D1 は `.wrangler/` の SQLite に保存される。Web Push を試すには、次の「セットアップ」の手順 2 で `.dev.vars` に VAPID の値を書く。

## セットアップ（初回のみ）

デプロイは GitHub Actions が行う。必要な設定は GitHub Secrets に置き、デプロイのたびに Worker へ渡される。

1. `gh auth login` と `npx wrangler login` を済ませる。
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
cargo check --target wasm32-unknown-unknown
```

pull request では `check`（fmt、clippy、test）と `wasm` が動く。main への push では、これらが通ったあとに Cloudflare Workers へデプロイし、`/manifest.webmanifest` が 200 を返すことを確かめる。

## ドキュメント

| ページ | 内容 |
|---|---|
| [docs/ci-cd.md](docs/ci-cd.md) | CI/CD の構成、GitHub Secrets の一覧、セットアップの詳細 |
| [docs/mvp0-web-push-verification.md](docs/mvp0-web-push-verification.md) | Web Push の検証記録と実機での手順 |
