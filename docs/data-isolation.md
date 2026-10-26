# 利用者ごとのデータ分離（Issue #57）

所有者はサーバーが認証済みセッションから決める `Member::owner()`（アプリ内利用者 ID）だけで、リクエストの引数に所有者は含まれない。

## 所有者条件の棚卸し

| データ | 所有者の持ち方 | 条件 |
|---|---|---|
| `medications` | `owner` 列 | 一覧・取得・保存・削除はすべて `owner=?` を含む。他人の ID を指定すると取得・保存は404、削除は409（変更なし） |
| `schedules`・`dose_slots`・`pause_periods` | `medications` への外部キー | 薬を所有者条件で解決した後にだけ書く。別の薬の枠 ID は奪取できない |
| `push_subscriptions` | `owner` 列 | 登録は同じ所有者の再登録だけ更新、別所有者の endpoint は409。状態確認・解除は `owner=?` |
| `push_probe` | `owner` 列（主キー） | 利用者ごとに1行。予約時は本人の有効なendpointを解決し、claim・完了はowner／generation／leaseで照合する |
| `notification_jobs` | `medications` 経由 | 薬の所有者に従う |
| `notification_deliveries` | ジョブと購読 | トリガー `delivery_owner` が、ジョブの薬の所有者と購読の所有者が異なる配信の作成を拒否する |
| `counters` | `owner` 列（主キー） | カウンターと `/saves` ストリームは利用者ごと。Hub は「保存があった」ことだけを全ストリームへ通知し、各ストリームが自分の値を読み直す |

共有する状態は `/clock` の現在時刻だけで、個人データを含まない。Hub に値は残らない。

## 既存の単一所有者データの引き継ぎ

#4 までのデータは `owner = 'owner'` で保存されている。最初にログインした利用者へ自動で割り当てることはしない。移行先は利用者が明示する。

事前に本番の既存 UUID を `D1_DATABASE_ID` に設定する。`cf` は DB 名での指定を受け付けない。

1. 移行先の人が Google でログインして、`users` に行を作る。
2. `scripts/legacy-owner.sh users --remote` で利用者の ID・作成時刻（UTC）・状態・所有する服薬数を表示し、移行先の行を選ぶ。ID はアプリの画面には表示しない。作成時刻が近い利用者が複数いるときは、移行先の人にログイン時刻を確認して取り違えを避ける。
3. Cloudflare ダッシュボードから対象の D1 を export してバックアップする（固定した `cf` には SQL export コマンドがない）。
4. `scripts/legacy-owner.sh status <USER_ID> --remote` で件数を確認し、`migrate` で引き継ぐ。
5. 切り戻しは `scripts/legacy-owner.sh rollback <USER_ID> --remote`。

`migrate` は、存在し有効な利用者にだけ、`medications` と `push_subscriptions` の `owner='owner'` の行を移す。移した行は `legacy_owner_migration` に記録され、`rollback` は記録された行だけを元の所有者へ戻す（その利用者自身の行には触れない）。再実行しても二重に移さない。実行前後の件数が表示される。

デモ用のカウンターは移行しない（利用者ごとに0から始まる）。送信待ちのテスト通知（`push_probe`）は移行しない。移行した購読を持つ利用者が通知設定をやり直すと再作成される。

## 既知の制約

- 同じブラウザで別の利用者がログインし、同じ Push endpoint を登録しようとすると、前の所有者が解除するまで409になる。
- 配信ジョブの生成・送信は `delivery`、テスト通知は `probe_delivery` が担当し、ownerを購読と照合する。スキーマ側の `delivery_owner` も維持する。実機受信・本番Cronの受け入れは [MVP受け入れ記録](mvp-acceptance.md)の未実施欄を参照。
