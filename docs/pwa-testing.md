# PWA の自動テストと本番ログ

## ローカル／CI

```sh
npm ci
npx --no-install playwright install chromium
npm run test:worker
```

Linux でブラウザのシステム依存が必要なら `playwright install --with-deps chromium` を使う。CI はこの手順を実行する。

`test:worker` は WASM をビルドし、使い捨て D1 と localhost:8791 の Worker を起動する。既存の HTTP テストに続き、`tests/pwa/` の Playwright テストを実行して、終了時にサーバーと D1 を破棄する。Google アカウント、本番 DB、実際の Push 購読は使わない。

確認する内容：

- 認証なしで取得できる manifest・アイコン・SW のヘッダーと、ページの manifest リンク。
- 通知許可だけでは保存済みにならないこと。
- ボタンから SW 登録、購読イベント、実際の hypersig Action、D1 保存、SSE による画面更新まで。
- 通知拒否時には SW 登録も購読保存もしないこと。
- Chromium の CDP 疑似 Push が実際の SW に届き、固定文言の通知が作成されること。
- SW 内に通知クリックイベントを注入すると通知が閉じられ、既存ウィンドウがある場合はその focus、ない場合は正しい URL の openWindow が要求されること。合成イベントには OS のユーザー操作権限がないため、この 2 API の呼び出し先はテスト内で記録する。

外部 Push サービスへの購読だけはブラウザ API をテストデータに置き換える。Push 配信は CDP で注入するため、FCM／APNs 経由の配信・暗号化解除はブラウザテストの対象外（暗号処理は Rust テストで別途確認）。OS による実際のクリック、新規ウィンドウ起動、iPhone のホーム画面への追加、アプリ終了中の通知表示は実機確認を残す。[実機検証手順](mvp0-web-push-verification.md#実機検証手順11)を参照。

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
| expired | 宛先が失効した（404／410） |
| retry | 再試行対象（429／5xx） |
| rejected | その他の HTTP 応答。`status` に応答コードを記録 |
| error | 暗号化・設定・通信など、Push 送信処理が失敗した |

購読 endpoint・鍵・利用者 ID・通知本文はこのログに含めない。送信予定がない Cron は実行ログのみで、Push 結果ログは出ない。ログの有効化だけでは自動アラートや端末からの受信確認は追加されない。

Workers Logs の保存・利用枠・料金は [Cloudflare の公式説明](https://developers.cloudflare.com/workers/observability/logs/workers-logs/)に従う。
