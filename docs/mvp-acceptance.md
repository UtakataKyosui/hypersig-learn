# MVP受け入れ記録（2026-10-26）

対象はIssue #25〜#32。仕様は#1、残作業の親は#79。旧工程#2・#6・#7の終了はMVP完成の証拠としない。外部環境の変更・本番デプロイ・実機操作はこの作業では実施していない。

## 実装と証拠の対応

| 条件 | 実装／自動確認 | 判定の範囲 |
|---|---|---|
| #25 期限付き生成・重複防止 | `src/delivery.rs::generate`、`test-scheduled.py`、`test-push-worker.mjs`、SQL試験 | 本番wasmのCron→ドメイン→D1で並行実行・重複、固定時計のJST日付境界・休薬開始・無効・保存直後・終了・15分ちょうど／1ms超過を確認 |
| #26 claim・lease・再試行 | 条件付きUPDATE、SQL複数接続、固定時計の本番wasm／D1／HTTP代替処理 | 通常通知とテスト通知の201／404／410／429／503／403、backoff後の回復、Push受付直後のD1保存障害、lease期限前の再送防止・期限後の回収、同じtag・減少するTTLを確認。旧完了／失効応答の拒否と試行回数上限はSQL・Rust試験も実施。実Pushサービスは未使用 |
| 編集・削除・休薬・解除後の抑止 | 認証付き本番Actions→D1→次のCron | 503後の未送信配信を編集／削除／無効化／休薬追加／購読解除で止めることを確認。外部HTTP開始後の撤回は不可。Push受付直後の保存障害では同じtagで再送されうることも再現 |
| #27 assets・通知クリック | assets HTTP試験、Chromium実SW合成Push・同一scope更新 | MIME・scope・no-cache・headリンク、固定本文、旧probe payload、実senderから復号したpayload、外部URL拒否、focus／openWindow境界、更新後のPush処理を確認。OS操作による実クリックは未実施 |
| #28 設定・照合・解除 | typed POST、Member、CustomEvent、Chromium試験 | 対応／許可／D1保存／端末エラー／起動形態を分離。非対応・拒否・許可済みで端末操作失敗・保存失敗・401と再ログインリンク・再読込照合・detail欠落・解除失敗からの復旧を確認。未登録と無効／失効を区別し、無効なD1購読の再登録も確認。ホーム画面起動はWebKit flagをシミュレートした表示試験で、実iOS確認ではない。予約は現在の端末の購読を使う。実機失効と実Google再ログインは未実施 |
| #29 全経路の結合 | `test-push-worker.mjs`、`test-scheduled.py`、Playwright | 画面→購読／予約Action→D1、Cron→本番wasm暗号化／VAPID→HTTP代替処理→復号、復号したpayload→実SWまで接続。D1保持はWorkerコード再読込後も確認。実FCM／APNsと実機を含む一続きの経路は未確認 |
| #30 Rust／Router／D1／wasm | 下記コマンドと既存テスト | Page初期値、Form、Patches、遷移、keyed差分、認証・所有者分離・原子更新・migration・エラー応答 |
| #31 モバイル・実機 | `medication-mobile.spec.ts`、下記の未実施表 | 幅390px・UTCのChromiumで日付／時刻blur、server rejectと入力保持、dirty解除、保存待ち中のEnter／ボタン再操作による二重送信防止、枠追加の時刻順・削除、JST表示・横幅を確認。iOS／Android実機は未完了。#11／#78の実機証跡と完成ビルドで照合が必要 |
| #32 設定・制約・引継ぎ | README、既存運用文書、本記録 | 手順と未達条件を記録。MVP完了宣言はしない |

Web Pushの暗号化は既存のRFC 8291ベクトルと署名試験に加え、実senderからのHTTP要求をNode標準cryptoで復号し、VAPID署名・aud・exp・TTL・payloadがURLとtagのみであることを確認する。`test-scheduled.py`は無効鍵の経路、`test-push-worker.mjs`はHTTP応答後の経路を担当する。後者は既存Wranglerが固定しているMiniflareを再利用し、新しいproduction依存は追加していない。時計だけをシミュレーターのentry wrapperで注入し、本番Rust／wasmにはテスト用APIを追加しない。すべての外向きHTTP要求を代替処理に渡し、実Pushサービスには接続しない。

HTTP結合試験の `scheduled: notification failure` は、Push受付直後にD1完了保存を意図的に失敗させたシナリオの期待ログ。この後のlease回収・D1状態・同じtagでの再送までassertし、予定外の失敗を成功扱いしない。復号した汎用payloadだけを一時ファイル経由でSW試験へ渡し、鍵・cookie・薬名・量は渡さない。実Pushサービスの受付・実機表示が成功した証拠にはならない。

テスト通知も `delivery::deliver` に接続し、残りTTL・固定payload・応答後の期限内backoffを通常通知と共有する。`0005_probe_delivery.sql`で予約のgeneration・lease・試行回数・完了状態を追加した。旧方式の「送信前に予約時刻を消して失敗時に戻す」処理は廃止した。SQLiteの別接続で並行claim拒否、lease回収、新予約への古い完了／失効応答の拒否、購読解除、最大試行回数・期限を確認する。切り戻しでは完了予約の時刻を消し、旧senderによる再送を防ぐ。

## 検証コマンド

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo check
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
python3 scripts/test-persistence.py
cargo check --target wasm32-unknown-unknown
npm ci
npm run test:worker
```

ローカルWorker試験は一時ディレクトリのD1とダミー設定を使い、終了時に削除する。`cf build`のrelease wasmと`cf deploy --prebuilt --dry-run --no-provision`も実施する。実デプロイはしない。sandboxではlocalhostのlistenが制限されるため、テスト実行には待受可能な環境が必要。Wranglerのログ先を一時領域へ変える場合は`WRANGLER_LOG_PATH`を使う。

2026-10-26の最終結果：`cargo check`、fmt、clippy、Rust 98テスト、SQLite 28テスト、wasm check、release wasmビルド、deploy dry-run、migration 5本、ローカルHTTP 78項目、固定時計の本番wasm／D1／HTTP応答結合（代替処理で27回のPush要求を検証）、通常時計のCron結合、Chromium 9試験がすべて成功。SQLiteには0005の前進・切り戻しと完了予約の再送防止も含む。Worker試験は`CARGO_NET_OFFLINE=true WRANGLER_LOG_PATH=/tmp/hypersig-wrangler-logs npm run test:worker`で実行した。Chromiumは合成Pushとクリック境界の代替処理を使い、OSによる実クリックや端末受信の成功には数えない。

初回のモバイル幅試験はdate pickerのfocusout操作とフォーム既定時刻の誤認で失敗した。合成Push試験にも不安定な失敗があったため、固有script URLから登録を特定し、activate完了とpushイベント到達を順に待ってから通知結果を確認するようにした。対象のみの4連続実行と、最終状態の全9件・Worker試験全体の成功を確認した。実機の未実施状態は変わらない。

## 旧工程 #2〜#7 からの引き継ぎ照合

2026-10-26に各Issue本文の完了条件・追加検証条件を再取得して照合した。下記は既存のチェック欄を書き換えたものではなく、この作業ツリーでの根拠と残条件である。

| 元の工程 | 現在の根拠 | 残条件 |
|---|---|---|
| #2 技術成立性・仕様確定 | wasm scheduled／静的assets／manifest headとヘッダー保持／許可操作・CustomEvent検証／保存失敗の表示。JS例外は既存AGENTS.mdの2ファイル内。設定・仕様はREADME | 閉じたiOS／Android PWAでの実Push縦通し。APIが使えることを実機成立性と同一視しない |
| #3 ドメイン・繰り返し・休薬 | `domain`・`medication_input`・`schedule_view`の固定時計試験。正の小数量、厳密日付、毎日・曜日・N日、両端を含む休薬と元の基準日での再開、400年暦往復、UTC／JST、保存前cutoffを確認。画面とCronで同じdomainを使用 | 実機の最終表示は#31へ引き継ぎ |
| #4 D1・原子更新・アクセス制御 | migration／rollback、別接続SQL、Router 400／401／403／415、所有者違反、Form再初期化、版更新・取消の原子性、実認証付きActionsとWorkerコード再読込後のD1保持 | 実アカウント・Safari／ホーム画面認証と本番複数isolateの運用確認。ローカル再読込を本番多拠点試験とはしない |
| #5 モバイル設定UI | 型付きSignals／Routes／Actions、Form／Validate、keyed list、共通予定計算、日付時刻helper、エラー・空状態・削除確認。Chromiumで入力保持・dirty解除・二重送信防止・順序／削除／再読込を確認。複数タブ同期は未採用 | iOS／Androidの一連の設定操作は#78／#31へ引き継ぎ |
| #6 通知統合 | 通常・テストの共通sender、並行Cron／重複／lease／失効／backoff／期限、認証付き変更後の抑止、解除復旧、401の再ログインリンク、SW更新／クリックURL制限。SSEなしのCron試験 | FCM／APNsと閉じた実機の配信・クリック。送信開始後の撤回不可とexactly-once非保証はREADMEに記録 |
| #7 回帰・実機・手順 | ドメイン、Page／Form／Patches／ListPatch、Router／D1、本番wasm、Chromiumの各層を実行。設定・復旧・秘密の扱い・通知制約・未確認条件をREADMEと本記録に記載 | HTTPSのiOS／Android実機受け入れは利用者の指示で後回し。MVP全体の完了条件は未達 |

## 未実施の最終受け入れ

2026-10-26、利用者の指示でHTTPS検証環境とiOS／Android実機での受け入れを後回しにした。以下の実機欄はすべて未実施。Chromiumで確認した項目も実機欄の成功へ転記しない。再開時は端末・OS・ブラウザ・ビルドSHA・試験日時を添えて結果を埋める。Pushサービス受付とOS通知表示は別欄で記録する。既存の[実機手順](mvp0-web-push-verification.md)も参照する。

| シナリオ | iOSホーム画面PWA | Android PWA |
|---|---|---|
| ホーム画面追加、許可／拒否、解除／再登録、失効復旧 | 未実施 | 未実施 |
| アプリを閉じた予定受信、通知タップ→当日予定 | 未実施 | 未実施 |
| 端末timezone UTCでJST予定と一致、薬名・量が通知に出ない | 未実施 | 未実施 |
| 日付／時刻blur・submit、server reject、入力保持・dirty解除・連打 | 未実施 | 未実施 |
| keyed list順序／削除、休薬・再開、複数枠表示 | 未実施 | 未実施 |
| Safari／ホーム画面認証、期限切れ、実アカウント間データ分離 | 未実施 | 未実施 |
| 通信断から復旧、古いSWから更新、閉じた状態で期限後復旧 | 未実施 | 未実施 |

仕様#1の保存・繰り返し・休薬・複数枠・認証・通常重複抑止は既存Rust／SQL／Router試験と今回の結合試験が根拠。実機通知・最終画面操作・本番Cronは根拠が不足している。したがって#1／#79と、旧工程#2（技術成立性）・#6（実配信結合）・#7（実機受け入れ）の引継ぎ条件をすべて達成したとは扱わない。#11／#78／#34の検証を含め、完成ビルドでの確認が必要。

## APIとJavaScriptの境界

hypersig／hypersig_ui 0.4.1を維持。mainガイド専用APIは使わず、Page::scriptはasset定数、manifest等は既存のhead挿入middlewareを使う。ブラウザに配信する独自JavaScriptは既存例外の`assets/sw.js`（Push表示・クリック・ライフサイクル）と`assets/push-subscribe.js`（ブラウザの許可・購読・解除・照合・起動形態の報告）に限定する。サーバーのURL・Actions・SignalsはRustの型付き定義から生成する。Node／Playwrightの試験用スクリプトは本番の配信assetに含めない。
