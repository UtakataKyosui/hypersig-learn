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
cargo check
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
python3 scripts/test-persistence.py
cargo check --target wasm32-unknown-unknown
npm run test:worker
```

Rustはrustupのproxyを使う（例：`export PATH="$HOME/.cargo/bin:$PATH"`）。別のRustを先に拾うと、rustupにwasmターゲットが入っていてもビルドできない。

## 通知を設定・復旧する

1. HTTPSのアプリにログインし、服薬設定で薬・服薬枠・繰り返しを保存する。予定はAsia/Tokyo固定。ホーム画面から「今日の予定」を確認する。
2. 通知設定を開く。iPhone／iPadではSafariからホーム画面へ追加し、そのアプリを開く。「通知を有効にする」を押して許可する。
3. 対応状況が「対応」、ブラウザの許可が「許可済み」、通知先が「保存済み」で、操作エラーがないことを確認する。許可だけでは設定完了にならない。保存に失敗した場合は通信を確認して同じボタンで再試行する。401では「ログインし直す」から別タブで再ログインし、戻って再設定する。
4. 「テスト通知を予約」を押す。ボタンを押した端末の購読を宛先として約1分後のCronが送信する。テスト予約は利用者ごとに1件で、新しい予約が前の予約を置き換える。予約受付・Pushサービス受付・実際の端末表示は別の結果なので、端末側で表示とタップ後の当日予定を確かめる。
5. 解除は「通知を解除する」を押す。ブラウザ購読とサーバー通知先の両方を解除する。通信断・ログイン切れの場合は再ログインして通知設定を開き直す。ブラウザに残した解除待ちの通知先を再送して復旧する。端末側の解除に失敗した場合は解除を再実行する。
6. 拒否した場合はOS／ブラウザの通知設定を変更する。「未登録」と「失効または解除済み」は別に表示する。どちらも「通知を有効にする」から設定し直せる。VAPID鍵を変更した場合は解除して購読を作り直す。非対応環境では対応ブラウザ・端末を使う。

「利用形態」は今開いている画面がホーム画面アプリかブラウザかを示す。ブラウザ表示だけではインストールの有無を断定しない。iPhone／iPadでは、追加済みの場合もホーム画面のアイコンから開き直す。

Service Workerは通知だけを扱い、服薬情報やページをオフラインキャッシュしない。再ログイン後も通知設定を開くとブラウザ購読とD1を照合する。端末／アカウントを変えたときはその端末ごとに設定する。

## 配信の制約

- CronはUTCで毎分起動し、Asia/Tokyoの予定を共通Rustドメイン計算から生成する。SSE・開いた画面・ブラウザタイマーは不要。
- 保存より前の予定、休薬中、終了済み、無効な予定は送らない。休薬期間の両端を含み、終了翌日以降の元の繰り返し条件に合う枠から再開する。
- 遅延／再試行は予定時刻から15分以内。期限切れを復旧時にまとめて送らない。TTLも残り時間以下にする。OSやネットワークによる表示の時刻は保証しない。
- 通常の重複生成・claimをD1で抑止する。leaseは2分、最大6回、再試行は送信応答後から60秒を初期値とする上限付き指数backoff。次の試行が期限に収まらなければ期限切れにする。1回のCronの配信処理は最大100件。未処理分は次のCronで期限内に処理する。
- テスト通知も同じ送信・TTL・再試行の判定を使う。D1に予約の世代とleaseを残し、途中停止から回収する。通常予定とテストは別キューでそれぞれ最大100件を処理し、一方の失敗だけで他方の処理を省略しない。
- 編集・削除・無効化・休薬追加は未送信の旧予定を抑止する。送信前確認と外部HTTP送信の間に変更が入る競合は残る。開始済みの送信とOS通知は撤回できない。
- 送信直後の障害では重複しうる。同じ予定の通知tagを再利用するがexactly-onceは保証しない。Push受付は端末表示や服薬済みを意味しない。服薬履歴は記録しない。
- 通知は「服薬の時間です」と固定表示し、薬名・量を含めない。タップ先は同一originの当日予定だけに制限する。

対応APIの存在と実機での動作確認は別。現状の自動確認はChromiumのみ。iOS／Androidの閉じたPWA受信・Safari認証を含む最終受け入れは未実施で、MVP全体を完了扱いしていない。[受け入れ記録](docs/mvp-acceptance.md)に根拠と残項目を記載する。

pull request では `check`（fmt、clippy、test）と `wasm` が動く。main への push では、これらが通ったあとに Cloudflare Workers へデプロイし、`/manifest.webmanifest` が 200 を返すことを確かめる。

## ドキュメント

| ページ | 内容 |
|---|---|
| [docs/ci-cd.md](docs/ci-cd.md) | CI/CD の構成、GitHub Secrets の一覧、セットアップの詳細 |
| [docs/medication-ui.md](docs/medication-ui.md) | 服薬設定画面の構成、状態表示、差分更新、複数タブ同期の採否、検証状況（#5） |
| [docs/cloudflare-cli-evaluation.md](docs/cloudflare-cli-evaluation.md) | `cf` CLI への移行記録、ローカル DB の引き継ぎと制約（#47） |
| [docs/mvp0-web-push-verification.md](docs/mvp0-web-push-verification.md) | Web Push の検証記録と実機での手順 |
| [docs/mvp-acceptance.md](docs/mvp-acceptance.md) | #25〜#32の実装・回帰結果と未実施の実機条件 |
