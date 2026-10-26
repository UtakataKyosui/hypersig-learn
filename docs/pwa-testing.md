# PWA の自動テストと本番ログ

実機と本番の自然起動Cronは [受け入れ手順・記録票](mvp-device-runbook.md)で別途確認する。ローカル試験の成功をその結果へ転記しない。

## ローカル／CI

```sh
npm ci
npx --no-install playwright install chromium
npm run test:worker
```

Linux でブラウザのシステム依存が必要なら `playwright install --with-deps chromium` を使う。CI はこの手順を実行する。

`test:worker` は WASM をビルドし、使い捨て D1 と localhost:8791 の Worker を起動する。既存の HTTP テストに続き、`tests/pwa/` の Playwright テストを実行して、終了時にサーバーと D1 を破棄する。Google アカウント、本番 DB、実際の Push 購読は使わない。

fixtureのSQLはmigration済みの使い捨てSQLiteへ直接実行し、CLI起動の繰り返しを省く。`FIXTURE_SQLITE=0`でcf経由に戻せる。WorkerのHTTPやCronの処理は代替しない。Playwrightの引数も渡せるため、合成Push試験の反復は次のコマンドで行える（build・HTTP・Cron結合試験も実行する）。

```sh
npm run test:worker -- tests/pwa/notifications.spec.ts --grep 'a synthetic push' --repeat-each=10
```

`Notification.close()`は同期の削除完了を保証しないため、通知一覧が空になるまで待ってから次のPushを注入する。各Pushの受信回数も検証し、単なる待ち時間の延長や自動retryで失敗を隠さない。

確認する内容：

- 認証なしで取得できる manifest・アイコン・SW のヘッダーと、ページの manifest リンク。
- 通知許可だけでは保存済みにならないこと。
- ボタンから SW 登録、購読イベント、実際の hypersig Action、D1 保存、SSE による画面更新まで。
- 通知拒否時には SW 登録も購読保存もしないこと。
- 現在の端末の購読を使ったテスト通知の予約Actionと、受付結果の表示。予約は端末表示成功と区別する。
- ローカルCronで服薬通知とテスト通知がD1からclaimされ、無効なテスト鍵による失敗後はbackoff待ちになり、重複Cronで直ちに再試行されないこと。
- `scripts/test-push-worker.mjs`で本番wasmを一時D1と外向きHTTPの代替処理につなぎ、固定時計のJST境界・並行Cron・201／404／410／429／503／403・減少するTTL・VAPID署名とpayload復号・受付直後のD1障害とlease回収・期限後復旧を確認する。実際の認証付きActionsで編集／削除／無効化／休薬／解除後の未送信通知抑止とWorker再読込後の保持も確認する。
- HTTP代替処理で復号した汎用payloadを一時ファイル経由でChromiumへ渡し、実SWの通知内容と同一scope更新後のPush処理を確認する。CDPは対象script URLから登録を特定し、activateとpushイベント到達を待ってからNotification APIの結果を確認する。
- 対応状況・許可・D1保存・端末操作エラーの分離、401の再ログインリンク、保存待ち中のEnter／ボタン再操作による二重送信防止。
- 未登録と無効／失効の区別、無効な購読の再登録、ホーム画面起動状態を独立して表示すること。WebKitのstandalone flagは試験内でシミュレートし、実iOS受け入れとは分ける。
- Chromium の CDP 疑似 Push が実際の SW に届き、固定文言の通知が作成されること。
- SW 内に通知クリックイベントを注入すると通知が閉じられ、既存ウィンドウがある場合はその focus、ない場合は正しい URL の openWindow が要求されること。合成イベントには OS のユーザー操作権限がないため、この 2 API の呼び出し先はテスト内で記録する。

外部 Push サービスへの購読だけはブラウザ API をテストデータに置き換える。Push 配信は CDP で注入するため、FCM／APNs 経由の配信・暗号化解除はブラウザテストの対象外（暗号処理は Rust テストで別途確認）。OS による実際のクリック、新規ウィンドウ起動、iPhone のホーム画面への追加、アプリ終了中の通知表示は実機確認を残す。[実機検証手順](mvp0-web-push-verification.md#実機検証手順11)を参照。

Miniflareは既存Wranglerに含まれる固定版を使う。固定時計のwrapperとHTTP代替処理はシミュレーター内だけにあり、本番のテスト用APIや外部ネットワークへの送信は追加しない。`scheduled: notification failure`が1回出るのは受付直後のD1保存障害を注入した期待動作で、その後の回収までassertする。最終結果は[MVP受け入れ記録](mvp-acceptance.md)に残す。

失敗時のスクリーンショットと trace は `test-results/` に保存され、CI では artifact として 7 日間取得できる。テストの trace には使い捨て認証 cookie と公開テスト購読が含まれうるため、本番に向けて実行しない。

## Cloudflare Workers Logs

`cloudflare.config.ts` で Workers Logs、実行ログ、保存を有効にしている。サンプリング率は 1（全件）。OAuth コードを含む URL クエリはログから除去する。

設定は既存の CI デプロイ後に反映される。ローカルからデプロイやリモート設定変更は行わない。Cloudflare Dashboard の Worker `app` → Observability → Logs で Cron 実行と例外を確認する。

Push を試行すると次のオブジェクトを出力する：

```json
{"event":"push_probe_result","outcome":"accepted","status":null}
```

`event = push_probe_result` で絞り、`outcome` を確認する。

| outcome | 意味 |
|---|---|
| accepted | Push サービスが受け付けた。端末表示の保証ではない |
| subscription_expired | 宛先が失効した（404／410） |
| retry | 期限内の再試行対象（429／5xx、暗号化・設定・通信失敗） |
| rejected | その他の HTTP 応答。`status` に応答コードを記録 |
| deadline | 次の試行が15分の期限に収まらない、または送信直前に期限切れ |
| attempt_limit | 最大試行回数に到達 |

購読 endpoint・鍵・利用者 ID・通知本文はこのログに含めない。送信予定がない Cron は実行ログのみで、Push 結果ログは出ない。ログの有効化だけでは自動アラートや端末からの受信確認は追加されない。

Workers Logs の保存・利用枠・料金は [Cloudflare の公式説明](https://developers.cloudflare.com/workers/observability/logs/workers-logs/)に従う。
