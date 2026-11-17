# 実装・検証結果

検証日: 2026-11-17（JST）。`judge/SPEC.md` に基づく実装とローカル検証を完了しました。配備、GitHub App の作成、アカウント操作、本物の git commit / push は実行していません。

## 実装した範囲

- `judge/`: TypeScript Worker、ブラウザと共通の replay、GitHub App の OAuth、署名付きセッション、同一 Origin 検査、D1 スキーマ、レート制限、厳密な短縮判定、記録・履歴 API、日英の提出ページ、ダークモード、テスト、設定、秘密の例、配備手順を含む日本語 README。
- GitHub への書き込みは UUID を検査した `records/submissions/<UUID>.json` の新規作成だけ。installation token を使い、App bot を作者にし、指定形式の Co-authored-by を付けます。ユーザーの名前・メール・ユーザートークンは保存しません。
- D1 の単一 INSERT でレート制限と採否を決めます。同手数は不採用で、不正な手順も記録します。GitHub 送信前の採用済み提出も現在の上界に含めます。失敗した送信は D1 に残り、短期リースで直列化してリクエスト後・5 分ごとの Cron で再送します。
- `tools/accept_submissions.py`: 標準ライブラリと既存 Python replay で再検証し、古い順に採否を決め、指定の TSV・提出 JSON・論文の上界セルを更新します。同一ミリ秒は D1 の sequence で順序を保ちます。処理済み提出の再実行は変更を生みません。
- `.github/workflows/accept.yml`: 提出ファイルへの push で判定、既存 verify_all / build_site、bot の更新を実行する定義です。権限は contents: write のみ。push 競合時は pull --rebase と再計算を行い、push は最大 3 回です。提出内容をシェルへ展開しません。
- `tools/build_site.py` は upper_kind() 内への 5 行追加だけです。外部提出（submission）の英語表示は `submitted by @login` になります。既存の英日ラベルを返すインターフェースは保っています。
- `records/submissions/.gitkeep` を追加しました。docs/ に提出先リンクは追加していません。

## 検証結果

| 検証 | 結果 |
|---|---|
| judge/ 内で npm install | 成功。依存を固定し package-lock.json を作成。最終インストール時の監査は 0 vulnerabilities |
| npm run types | 成功。Wrangler から src/env.d.ts を生成 |
| npm run typecheck | 成功。TypeScript エラーなし |
| npm test | 2 ファイル・22 テスト成功。最終実行 13.50 秒 |
| Python replay との照合 | solutions/best.tsv の全 13 行と、各行の破損例 6 種類、計 91 ケースで一致 |
| PYTHONDONTWRITEBYTECODE=1 python3 tools/test_accept_submissions.py -v | 10 テスト成功。最終実行 9.026 秒 |
| wrangler d1 execute colorsort-judge --local --file schema.sql | ローカルのみ、6 コマンド成功 |
| wrangler dev --local --ip 127.0.0.1 --port 18787 --inspector-port 18788 | ローカル起動成功 |
| curl GET / | 200、text/html; charset=utf-8。CSP・no-store ヘッダーを確認 |
| curl POST /api/submit（同一 Origin、未ログイン） | 401、`{"error":"authentication"}` |
| npm run test:browser | 4 組み合わせ、48 アサーション成功。JavaScript エラーなし |
| git diff --check | エラーなし |

Worker テストは実際のローカル Workers/D1 環境で実行し、GitHub fetch をモックしました。

- HMAC の署名と改ざん検出、異なる鍵、壊れた Cookie、未来の発行時刻、7 日の期限境界。
- OAuth の scope なし認可 URL、state と Cookie の照合、10 分の期限、一度限りの消費と競合、code 交換失敗、ユーザートークンと個人情報を残さないこと。
- Origin 欠落・null・別オリジン・紛らわしいホスト名の拒否、ログアウト時の Cookie 失効。
- 25 件の同時提出で、採用 1 件・同手数の不採用 19 件・件数制限 5 件となること。ユーザーごとの分離、1 日 100 件、期限を過ぎた行の扱い。
- 不正・同手数の記録、入力の正規化、余分なフィールド・不正な型・巨大な本文の拒否、D1 の未反映記録を含めた最小上界と先着保持者。
- その場で生成した RSA 鍵による RS256 JWT と公開鍵検証、改ざん検出。
- 書き込みパス制限、App bot の作者指定、Co-authored-by、送信失敗後の保持・順序・再送、成功応答だけ失われた場合の回復。

Python テストは必要なファイルを一時ディレクトリにコピーして実行しました。採用→短縮→同手数、古い順序、metadata・手順・宣言手数の検査、未対応 Unicode、壊れた JSON、symlink 拒否、再実行の冪等性、論文の本文・下界を変えないことを確認しました。採用後の verify_all.py --readme と build_site.py も写しの中で成功しました。Actions の通常成功・push 競合・rebase 競合・3 回での終了は、本物の git を一切呼ばない偽コマンドで確認しました。

ブラウザは独立した空のヘッドレス Chrome で、日英・明暗・PC 幅 1100px / モバイル幅 390px・未ログイン / ログイン後の 4 組み合わせを確認しました。認証・提出 API はモックです。即時 replay、不正手の番号、送信可否、新記録・同手数の結果、保持者・履歴、言語切り替え、ページ全体の横はみ出しがないことを確認しました。画像も目視確認しました。

- [日本語・PC・ログイン後](test/artifacts/ja-light-signed-in.png)
- [英語・モバイル・ダーク・ログイン後](test/artifacts/en-dark-signed-in.png)
- [日本語・未ログイン](test/artifacts/ja-light-anonymous.png)
- [英語・未ログイン](test/artifacts/en-dark-anonymous.png)

実ブラウザ検証で、Wrangler の関数名保持が生成する補助関数の参照切れを検出し、keep_names = false に修正して確認を通しました。不正な Unicode を結果 JSON に書くときも、安全にエスケープするよう修正し、回帰ケースを通しました。

## 環境と実行していないこと

最終確認時: Node.js v26.13.0、npm 11.20.0、Wrangler 4.163.0、Vitest 4.1.12、Cloudflare Vitest plugin 1.8.3、TypeScript 7.0.2、Playwright 1.65.1。依存バージョンは package.json と lockfile に固定しています。

サンドボックス内の編集ヘルパーと Python が macOS のタイムゾーン初期化で待機したため、許可されたファイルの編集・ローカル検証は通常権限で実施しました。終了したのはこの作業が起動した待機中ヘルパー・Python、独立したヘッドレスブラウザ、検証用 Wrangler だけです。既存の colorlb・big・iterate_big.py・colorsat.py、他の beam/・lowerbound/ 作業は停止・変更していません。

今回のファイル編集は仕様の許可範囲に限定しました。実リポジトリの bounds.tsv・solutions/・README.md・README.ja.md・docs/ はこの作業では更新せず、更新処理は一時コピーで検証しています。

本物の GitHub OAuth、installation token の発行・Contents API の書き込み、GitHub Actions 上の commit / push、リモート D1、Worker の配備は実行していません。これらは単体・統合テストのモック範囲です。GitHub App の登録、D1 id の仮値の置き換え、6 個の secret 登録、Actions/Pages 設定の確認が配備時に必要で、手順は README.md に記載しています。作業中の一時的な自動承認審査の拒否は、SPEC.md の明示的な作成指示を再提示して解消しており、未解消のブロックはありません。
