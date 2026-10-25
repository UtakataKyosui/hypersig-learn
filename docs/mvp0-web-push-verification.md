# MVP 0: Web Push／PWA の技術検証と仕様確定

Issue #2（親 #1）の検証結果。`hypersig` / `hypersig_ui` 0.4.1、`worker` 0.8.7 が基準。

## 検証状況

| 項目 | 状態 | 根拠 |
|---|---|---|
| Web Push 暗号化（RFC 8291）が RFC の付録テストベクタと一致 | 検証済み | `push::tests::matches_the_rfc_8291_example`（`cargo test`） |
| VAPID JWT（ES256）が公開鍵で検証できる | 検証済み | `push::tests::vapid_token_verifies_with_the_advertised_key` |
| 暗号系依存が `wasm32-unknown-unknown` でビルドできる | 検証済み | `cargo check --target wasm32-unknown-unknown`、`sh scripts/build.sh` |
| `scheduled` 入口から Push サービスへ HTTP 送信できる | ローカル検証済み | `wrangler dev --test-scheduled`。実在しない endpoint を保存し、予定時刻後の `/cdn-cgi/handler/scheduled` で FCM が応答（`Expired`）。予定時刻前は送信しない。2 回続けて呼んでも送信は 1 回 |
| manifest の `<link>` を `<head>` に挿入し、元のヘッダーを保つ | ローカル検証済み | `curl` で `<link rel="manifest">` を確認。単体テストでヘッダー保持と `Content-Length` 除去を確認 |
| `/sw.js` が `Service-Worker-Allowed: /` 付きで配信される | ローカル検証済み | `curl -i` |
| `CustomEvent<SubscriptionInput>` → `REGISTER.call_with` の配線 | ブラウザで検証済み（localhost） | プレビューブラウザで `push-subscription` を発火：不正 endpoint は「通知の宛先が不正です。」を表示し「未保存」のまま、正しい購読は「保存しました」に変わる。`push-status` の `denied` が「ブラウザの許可」に反映され、許可だけでは保存済みにならない |
| manifest の読み込みと SW の scope | ブラウザで検証済み（localhost） | manifest を取得でき（scope `/`、`standalone`）、`/sw.js` を scope `/` で登録して active になった。`<push-subscribe>` が定義されボタンが出る |
| 偽造・欠落・不正鍵のリクエストが成功扱いにならない | 検証済み | `curl`：不正ホスト・不正鍵は 400、`detail` 欠落（`{}`）は 400、購読なしの予約は 409 |
| iPhone ホーム画面 PWA／Android で閉じた状態の受信と通知クリック | **未検証（実機が必要）** | 下の「実機検証手順」 |
| `wrangler deploy` した Worker の Cron 実行 | **未検証** | ローカルは `--test-scheduled` で呼んだのみ |

Push サービスが `Accepted` を返すこと（送信成功）と、端末が通知を表示することは別である。実機で表示を確認するまで #2／#11 は完了にしない。

## 採用構成

```
ブラウザ <push-subscribe>  ──許可（ボタン直接操作）→ SW 登録 → 購読
        │ push-subscription (endpoint, p256dh, auth)
        ▼ on_custom → REGISTER.call_with(detail)
Worker  validate(): https＋許可 Push ホスト＋鍵の形式 → D1 保存 → Patches(saved=true)
        scheduled(毎分) → D1 から期限内の予定を条件付き UPDATE で claim → push::send
        push::send: RFC 8291 暗号化＋VAPID → Fetch POST → Outcome
Push サービス → SW(push) → showNotification → クリックで /notifications を開く
```

| 要素 | 場所 |
|---|---|
| asset のパス定数・manifest・head 挿入 | `src/assets.rs` |
| Web Push 暗号化・VAPID・送信 | `src/push.rs` |
| 設定画面・購読保存 Action・scheduled 処理 | `src/notifications.rs` |
| Service Worker／購読 custom element／アイコン | `assets/` |
| Cron・VAPID 設定 | `wrangler.toml` |

### 0.4.1 固有の不足と回避

| 不足 | 回避 |
|---|---|
| `Page` に manifest／任意 head の API がない | `axum::middleware::map_response` で `text/html` の本文の最初の `</head>` の前に固定のタグを挿入する。`Content-Length` だけ除き、他のヘッダーは保つ。Worker 側で圧縮していないため圧縮前に処理できる |
| `Page::script` は `&'static str` | `assets.rs` の定数を使う |
| `Action` の応答は JSON ではなく Patches（SSE 形式） | custom element は保存結果を読まない。保存済みかどうかは、サーバーが `saved` signal を patch したときだけ画面に出る |
| `routes!` の関数・定数は `pub` にできない | ページの Route 宣言は `src/lib.rs` に置く |
| 許可状態（`permission`）と購読保存（`saved`）の混同 | 別 signal にした。`granted` でも `saved` は `false` のまま |

### 依存（承認済み）

`p256`（0.13、`ecdh`／`ecdsa`）、`aes-gcm` 0.10、`hkdf` 0.12、`sha2` 0.10、`base64` 0.22、`rand_core` 0.6（`getrandom`）、`getrandom` 0.2（`js`）、`serde_json`、`js-sys`。

- `p256` は 0.14 系が `getrandom` 0.4 と `rand_core` 0.10 を引き込み wasm で失敗するため、`aes-gcm` と揃う 0.13 系に固定する。
- `getrandom` 0.2 の `js` は AGENTS.md の指示どおり必須。

## JavaScript 境界（AGENTS.md の例外）

「独自 JavaScript 禁止」の例外として、次の 2 ファイルだけを承認済みとする。Service Worker は custom element では代替できない。

| ファイル | 責務 | 持たせないもの |
|---|---|---|
| `assets/sw.js` | `push` 受信と `showNotification`、`notificationclick` で画面を開く | 業務ロジック、薬名・量の保持 |
| `assets/push-subscribe.js` | `<push-subscribe>`：ボタン操作での許可要求、SW 登録、購読、`push-status`／`push-subscription` イベントの発行 | サーバー応答の解釈、`fetch`、状態の保存 |

- 契約：属性 `vapid-key`／`worker`／`scope`／`label`、イベント `push-status`（文字列）と `push-subscription`（`{endpoint, p256dh, auth}`）。名前は Rust 側の `PUSH_STATUS`／`PUSH_SUBSCRIPTION` と `assets.rs` の定数で一箇所に持つ。
- DOM イベントの実データは型宣言だけでは保証できないため、`notifications::validate` がサーバーで再検証する。
- 通知本文は `sw.js` に固定文言として書いてある（現状は薬名・量を含めない）。#6 で本文を payload から取る場合は、薬名・量を既定で出さない方針（#1）を保つ。

## 仕様の確定

#1 の初期案を、本検証で覆す理由がないため確定として扱う。量の表現のみ追加で決めた。

| 項目 | 内容 | 区分 |
|---|---|---|
| 利用範囲 | 本人 1 人の非公開 MVP。一般公開・複数利用者・家族共有は対象外 | 確定（#1） |
| 対象端末 | iPhone（iOS 16.4 以降のホーム画面 PWA）と Android の対応ブラウザ | 確定（#1）。実機は未検証 |
| タイムゾーン | `Asia/Tokyo` 固定。画面に明示し、端末の設定変更で予定を変えない | 確定（#1） |
| 繰り返し | 毎日／曜日（複数）／N 日ごと（N≥1）。開始日必須、終了日任意（両端を含む）。N 日ごとは開始日からの暦日差で判定し、休薬を挟んでも基準日をずらさない | 確定（#1） |
| 休薬 | 薬ごとに開始日・終了日（両端を含む終日）を複数登録。期間内は全枠を抑止し、翌日以降の最初の該当枠で再開。重複は入力エラー。周期的な「N 日服薬＋M 日休薬」は対象外 | 確定（#1） |
| 遅延期限 | 予定時刻から 15 分以内のみ送信・再試行。超過は期限切れ。検証コードの `DEADLINE_MS` と Push の `TTL`（900 秒）も同じ値 | 確定（#1） |
| 量 | 正の小数、小数第 2 位まで、0.01〜1000、保存は 100 倍した整数。浮動小数は使わない | 確定（本 Issue） |
| 長さ上限 | 薬名 100 文字、単位 20 文字、メモ 200 文字 | 確定（本 Issue） |
| 通知本文 | 既定は「服薬の時間です」。薬名・量は出さない | 確定（#1） |
| 送信成功の扱い | Push サービスの受付と、端末表示・服薬済みを区別する。受付を服薬済みにしない | 確定（#1） |

### 認証方式

| 案 | 評価 |
|---|---|
| A. `tower-sessions` 0.14 ＋ D1 の自作 `SessionStore` | 公式の D1／KV ストアがなく、0.14 の `SessionStore` を Workers 向けに自作し wasm 互換を確認する必要がある。単一利用者には過剰 |
| B. Cloudflare Access を前段に置く | コード不要。ただし manifest は認証情報なしで取得されるため、保護下では PWA の manifest 取得が失敗しうる。Push の送信自体には影響しない |
| C. Worker secret のパスワードで署名した Cookie（無状態） | ストア不要。`/manifest.webmanifest`、`/sw.js`、アイコン、`push-subscribe.js` を認証対象外にでき、ページはリダイレクト、Action は 401 にできる（`RequestKind::of`） |

採用案は **C**（#3 で実装）。手順は次のとおり。

- 署名には HMAC（`hmac` の追加が必要。追加時は事前承認を得る）と Worker secret を使う。
- 期限付き Cookie を発行し、`HttpOnly`／`Secure`／`SameSite=Lax` とする。
- パスワード照合は定数時間比較にする。

保留：Cookie の有効期限とパスワード変更時の無効化方法は #3 で決める。

### 本検証コードのセキュリティ上の制約

- 購読登録・予約 Action に認証がない。検証用に限定して使い、**公開 URL へ置く間は検証後に削除する**。#3 の認証を入れるまで本番相当の運用はしない。
- 購読は 1 行のみ保持し、上書きされる。宛先は Chrome／Firefox／Safari／Edge の Push ホストに限定して検証する（任意ホストへの POST を防ぐ）。
- VAPID 秘密鍵は Worker secret（`VAPID_PRIVATE_KEY`）にだけ置く。ローカルの `.dev.vars` は `.gitignore` に入れてある。ログには結果の分類のみ出し、endpoint・鍵・本文は出さない。

## 実機検証手順（#11）

実機の縦通しは人が行う。結果は次の表に記録する。

### 準備

1. `scripts/setup-vapid.sh` で VAPID 鍵対と連絡先を設定する。秘密鍵と連絡先は Worker secret に入り、公開鍵は `wrangler.toml` に書かれる（詳細は `docs/ci-cd.md`）。秘密鍵をチャット・Issue・コミットに貼らない。
2. `npx wrangler deploy`（`workers.dev` の HTTPS）。

### 手順

1. 端末で `/notifications` を開く。iPhone は Safari の共有メニューから「ホーム画面に追加」して**ホーム画面のアイコンから**開く。
2. 「通知を有効にする」を押して許可する。画面の「ブラウザの許可」が `granted`、「購読をサーバーに保存しました。」が出ることを確認する。
3. 「1 分後にテスト通知を送る」を押す。
4. アプリを閉じる（iPhone はマルチタスクから終了）。
5. 約 1〜2 分後に OS 通知が出ることを確認し、タップして `/notifications` が開くことを確認する。
6. `npx wrangler tail` で `push probe: Accepted` を確認する。

### 結果記録

| 端末・OS・ブラウザ | 日時 | Push 受付 | 端末の表示 | クリックで復帰 | 備考 |
|---|---|---|---|---|---|
| （未実施） | | | | | |

## 後続への引き継ぎ

- #3：認証（案 C）。`assets` のパスは認証対象外にする。
- #4：`push_probe` の暫定テーブル（リクエストごとの `CREATE TABLE IF NOT EXISTS`）を migration と `push_subscriptions`／`notification_*` に置き換える。
- #6：`push::send`、`Outcome`（Accepted／Expired／Retry／Rejected）、`validate` を配信処理に使う。
- ローカルで `cargo` を使うときは `~/.cargo/bin` を PATH の先頭に置く。Homebrew の `cargo` は wasm32 ターゲットを持たない。
