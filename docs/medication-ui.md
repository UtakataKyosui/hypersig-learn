# 服薬設定画面（Issue #5 / #21〜#24）

薬・服薬枠・休薬期間の設定画面。計算は [medication-domain.md](medication-domain.md)、保存は [persistence-auth.md](persistence-auth.md) を参照。

## 構成

| ページ | モジュール | state | 内容 |
| --- | --- | --- | --- |
| `/medications` (`MEDICATIONS`) | `medication_list` | `Settings` | 今日の予定、全体の次回通知、薬ごとの状態、薬の削除 |
| `/medications/new` (`NEW_MEDICATION`)、`/medications/{id}` (`MEDICATION`) | `medication_edit`、`pause_edit` | `EditPage` | 薬・繰り返し、服薬枠、休薬期間、状態の帯 |

未ログインの `/medications...` はログインへ redirect し、Action は 401 を返す（`auth.rs`）。

| Action | ハンドラ | 内容 |
| --- | --- | --- |
| `SAVE_MEDICATION` / `SAVE_SLOT` / `REMOVE_SLOT` | `medication_edit` | 薬本体、服薬枠1件の追加・更新・削除 |
| `SAVE_PAUSE` / `REMOVE_PAUSE` | `pause_edit` | 休薬期間1件の追加・更新・削除 |
| `medication_list::REMOVE` | `medication_list::remove` | 薬の削除（枠・休薬ごと） |

編集系 Action はすべて、保存済みの薬を読み直し、自分の担当部分だけを差し替え、ページが最後に見た revision で `medications::commit` する。古い revision は 409。

## 状態と表示

`schedule_status::assess` が `schedule_view::query`（通知と同じ計算）から薬ごとの状態を返し、一覧と編集ページの両方が使う。複数に当てはまるときは 通知オフ → 終了 → 服薬枠なし → 休薬中 の順で優先する。

| 状態 | 一覧の表示 | 意味 |
| --- | --- | --- |
| `disabled` | 通知オフ | 通知が無効 |
| `ended` | 終了 | 終了日を過ぎた |
| `empty` | 服薬枠なし | 枠が1件もない |
| `paused` | 休薬中（開始 〜 終了） | 今日が休薬期間内。次回は休薬明け |
| `active` | 通知中 | 上のいずれでもない |

編集ページの帯は、今日が休薬期間内なら状態によらず休薬の文を出し、次回の文は上の状態から作る。一覧の「次回の通知」は全薬のうち最も早い予定。

### 時刻

画面の時刻はすべてサーバーが `schedule_view::tokyo_text` で作る文字列（`YYYY-MM-DD HH:mm`）で、Asia/Tokyo を併記する。UTC+9 を整数演算で足すため、サーバーやブラウザのタイムゾーンに依存しない（ブラウザ側で日時を整形しない）。今日の予定は時刻順、薬は名前順、休薬は開始日順。

## 差分更新

- 一覧の薬と今日の予定は `each_keyed` のキーで描く（薬 ID、`{薬ID}:{枠ID}`）。薬の削除は `items.patch_remove`、今日の予定と次回通知は再計算した値で上書きする。
- 服薬枠と休薬期間は、保存・削除のたびに時刻順／日付順に整えた全体を signal に送る。`patch_upsert` は末尾に追加するだけで並べ替えないため、順序を保つ今回は使わない。`each_keyed` はキーで行を保つので、並べ替えでも行は作り直されない。
- 一覧向けの `medications::SAVE`（画面からは未使用）は `patch_upsert` を使う。

## エラーと確認

- 入力の誤りは該当フィールドの `FieldErrors`（200）。通信と保存の失敗は `failure::alerts` が一覧・編集ページ共通で出す: 401（再ログインのリンク。入力は画面に残る）、応答なし（status 0。接続の確認）、その他の拒否（サーバーの文。例: 409 競合）。
- 削除は `hypersig_ui` の alert dialog で確認する（薬、服薬枠、休薬期間）。実行ボタンだけが Action を呼ぶ。

## 複数タブ同期（採否の記録）

**採用しない。** `StreamAction` + `.reconnect()` は使わない。

- `Hub` Durable Object は `saves` という1つの固定名で、カウンターの保存を無内容の1行で全購読者へ知らせるだけ。利用者・薬の区別も内容もないため、薬の変更の通知には使えず、利用者ごとの Hub 名と通知内容の設計が別途要る。
- 再接続時は、現在の一覧の再送、upsert では消えない削除済み行の除去、入力中のフォームを上書きしないことが必要になる。どれも小さくなく、通知の安全性（誤った利用者への漏れ）も絡む。
- 他タブの変更は、ページの再読み込みと保存時の revision 検査（409）で守られる。

再検討する条件: 複数端末での同時編集が実際に問題になったとき。その際は、利用者ごとの Hub、再接続時に一覧全体を置き換える（削除済み行の除去）、編集中の `Form` の `is_dirty` を見て上書きしない、を満たし、切断後の復元と入力中の保持をブラウザで確認する。

## 検証状況

実施: `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（`TZ=America/Los_Angeles` と `TZ=Pacific/Kiritimati` でも実行）、`cargo check --target wasm32-unknown-unknown`、`cf build`。

ローカルの `cf dev`（Chromium 系のプレビュー、ブラウザの TZ は Asia/Tokyo、利用者とセッションは `scripts/auth_fixture.py` で投入）で確認した: 薬の登録、枠の追加が時刻順に並ぶこと、枠の削除（取消と実行）、休薬期間の追加と次回日時の変化（連続する休薬の翌日まで進む）、一覧での状態表示（通知オフ・終了・休薬中）、休薬中で今日の予定が空の表示、薬の削除と空状態、401 の再ログイン表示とその後の成功。

未確認:
- ブラウザの TZ を Asia/Tokyo 以外にした表示（時刻はサーバー生成の文字列で、単体テストは TZ を変えて通る）。
- N日ごとの繰り返し、終了日の入力、409 競合の画面、通信断（status 0）の画面。
- スマートフォン、Safari、実機。
- 複数タブ（同期は採用しないため対象外）。

## 既知の制約

- 新しい枠の時刻順は保存後の全体送信で保たれるが、編集中の枠の時刻を変えても保存するまで並びは変わらない。
- 曜日指定のエラーは日曜のチェックボックスに付く。
- 単位は固定の選択肢。保存済みの単位が選択肢にない場合、編集フォームでは空欄に見える。
- 休薬期間のクライアント側の検証は空欄だけ。逆転・重複はサーバーが検証して該当欄に返す。
- 一覧は読み込み時点の内容。他タブでの変更は再読み込みで反映する。
