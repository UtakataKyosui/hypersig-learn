# D1・認証・保存契約（Issue #4 / #16〜#20）

## 起動とデプロイ

1. `.dev.vars` に `GOOGLE_CLIENT_ID`、`GOOGLE_CLIENT_SECRET`、`GOOGLE_REDIRECT_URL` を設定する。値はコミットしない。設定手順は [google-auth.md](google-auth.md)。
2. 従来のローカル DB を引き継ぐ場合は、先に [cf 移行記録](cloudflare-cli-evaluation.md) のコピー手順を実行する。`npm run db:migrate:local` を実行する。再実行は適用済み migration をスキップする。既存 counter / push_probe テーブルと値は保持する。
3. `npm run dev` で起動し、`/login` を開く。Cookie は `__Host-session`（セッション ID のみ）、HttpOnly / Secure / SameSite=Lax / Path=/。サーバー側の有効期限は24時間。本番は HTTPS で利用する。HTTP localhost での Cookie 保持は Arc で確認したが、Safari では保持されず再びログインへ戻った。Safari の開発確認には信頼済み証明書の HTTPS 環境が必要。

本番への適用は既存の main デプロイ CI が担当する。merge 前に GitHub Secrets `GOOGLE_CLIENT_ID`、`GOOGLE_CLIENT_SECRET`、`GOOGLE_REDIRECT_URL` を利用者が登録する。CI は秘密の存在とリダイレクト URL の形式を確認してから migration、Worker の順で適用する。この PR の作成時には本番 migration、secret 登録、デプロイを行っていない。

セッションは D1 の `sessions` に保存され、ログアウト Action は記録を削除して Cookie を消す。利用者単位の失効は `users.session_version` の加算または `status='disabled'` で行う。長時間接続中の stream は接続時に認証し、再接続時に再検証する。

## 保存境界

`src/repository.rs` が D1 アクセスを担当する。SQL の値はすべて bind する。ID は文字列、日時は Unix ミリ秒、日付・時刻は ISO ローカル表現（Asia/Tokyo）、量は100倍整数。`medications.document` は編集入力のスナップショットで、正規化テーブルと同じ batch で書く。読み出す版は常に `medications.revision` を正本とする。

`medications::SAVE` は `Editor` 全体（薬、繰り返し、複数枠、複数休薬、有効状態）を保存する。各要素の追加・編集・削除はこの集約を変更して保存する。新規は revision=0、編集は LOAD が返した版を送る。枠・休薬の ID を変更しない限り同じ要素として扱う。枠64件、休薬128件を上限とする。

版付き UPSERT の直後に `changes()` を同一 batch 内で検証する。競合時は SQL を失敗させて batch 全体をロールバックする。版更新 trigger が未送信旧ジョブと未完了配信を取消する。休薬重複はドメイン検証と DB trigger の双方で拒否する。別の薬の枠 ID は奪取できない。削除された枠／薬に属するジョブ・配信は外部キーで削除する。削除・変更前に送信開始済みの Push は撤回できない。

保存成功は `Form::new(saved)` と keyed upsert、削除は keyed remove。入力失敗は Form reject、所有者違反は404、競合は409、DB障害は固定文言の500。画面の操作部品・一覧描画は #5 配下で追加する。`/medications` は初期 state と typed Action の契約を供給する最小ページ。

## 認証・購読

全ページ／Action／stream を認証 middleware で保護し、各データ handler も Member extractor を必須とする。所有者は認証済みセッションのアプリ内利用者 ID で、入力には所有者フィールドを持たない。ページの navigation は `/login` へ移動し、Action / stream と直接 API 呼出は401。公開対象はログイン、Google ログイン開始と callback、固定の PWA／runtime assets のみ。認証後の応答は no-store。

POST の Sec-Fetch-Site、JSON Content-Type、引数 deserialize の拒否は hypersig 0.4.1 が担当する。Google への遷移と callback は Google が起こす GET ナビゲーションのため、typed Action ではなく通常のルートで、フローの `state` とブラウザの結び付けが CSRF 対策を担う。独自の更新用 JSON ルートは作らない。

SUBSCRIPTION_STATUS は本人の endpoint に対する有効状態だけを返す。購読は HTTPS の許可 Push ホストと P-256／auth 鍵を検証する。endpoint ごとに一意、同じ所有者の再登録は鍵更新・再有効化、別所有者への移管は拒否する。UNREGISTER は本人の endpoint 文字列を受けて無効化し、未完了配信と予約済み probe を取消する。404/410 による失効も無効化する。送信時の redirect は追わず、鍵が差し替わった購読を古い応答で失効させない。

購読鍵は画面へ返さない。DB・送信障害は固定の分類のみログに残し、SQL、endpoint、鍵、薬名、量は出力しない。`push_probe` は技術検証用テスト通知の予約で、利用者ごとに1行を持ち、保存済みかつ有効な本人の購読だけを送信対象とする。服薬ジョブの生成・claim・再試行は #6 配下が担当する。

## 検証

- `cargo check`、fmt、clippy、Rustテスト、wasm check、worker-build を実行。
- `python3 scripts/test-persistence.py` は production の SQL を SQLite で実行し、独立した接続からの読み出し、競合／重複失敗の全ロールバック、版更新と取消、所有者、購読再登録／無効化、FK／一意制約を確認する。CI でも実行する。
- 一時ローカル D1 への migration 適用・再適用を Wrangler 4.152.0 で確認。
- ローカル Worker HTTP でログイン、Cookie 属性、CRUD再読込、競合、別薬の枠奪取拒否、400/401/403/415、購読再登録・解除を確認。
- `scripts/test-worker.py` は localhost:8791 の使い捨て環境専用。Google の設定値をダミーで与えて起動し、同じローカル D1 に migration を適用してから実行する。Google アカウントは使わず、`scripts/auth_fixture.py` が利用者とセッションをローカル D1 へ投入する。実在しない購読を保存するが、通知送信は行わない。ログイン開始の redirect、callback の不正入力と再利用、旧 Cookie の拒否、2利用者間の分離、失効、ログアウト、公開 manifest を含む45項目を実行した。
- ネイティブブラウザの UI 操作で確認：Safari で未認証ページからログインへ移動、パスワード入力 binding と不正入力時の401表示。Arc で正しい入力からログインに成功し、服薬設定ページの表示と再読込後の認証保持を確認。テスト用パスワードはブラウザに保存していない。
- 購読失効についても SQL テストで、古い鍵への遅延した失効応答が差し替え済みの購読を無効にしないことと、現在の鍵に対する失効が配信取消まで反映されることを確認。
- 実際の端末 Push と本番 Cron、実際の複数 isolate での試験は後続工程として未実施。Rust の Router テストと HTTP リクエストごとに新規作成される AppState、SQLite の別接続を使って永続性の境界を検証した。

設計根拠: [D1 batch の原子性](https://developers.cloudflare.com/d1/worker-api/d1-database/#batch)、[D1 migrations](https://developers.cloudflare.com/d1/reference/migrations/)。hypersig / worker API は Cargo.lock の 0.4.1 / 0.8.7 のローカルソースで照合。
