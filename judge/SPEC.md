# 上界の記録を受け付けるオンラインジャッジ（仕様）

## 目的
誰でも GitHub でログインして、色そろえパズルの手順を提出できるサイトを作る。今の上界より短ければ自動で記録になり、
リポジトリ（`bounds.tsv`・`solutions/best.tsv`）と公開サイト（GitHub Pages、`docs/`）に反映される。

## パズルと判定（今の `tools/verify_all.py` の `replay()` と完全に同じ規則）
- N 色、各色 N 個、レーン N+1 本（0..N）、容量 N。初期状態はレーン 0 が空、レーン 1..N がどれも下から 1..N。
- 手順は 1 手 2 文字（移動元・移動先のレーン番号を base36 "0123456789abcdefghijklmnopqrstuvwxyz" で）。
- 1 手: 移動元が空でなく、移動先が満杯でなく、移動元≠移動先、番号が N 以下。
- 完成: どのレーンも空か、同じ色 N 個。
- 受け付ける色数は **2〜24**。手順の文字数は偶数、手数は 1〜5000、文字は base36 小文字だけ（前後の空白・改行は除く）。
- 採用の条件: **今の上界より真に短い**こと（同じ手数は先に出した人の記録のまま。受け付けて記録はするが採用しない）。
  今の上界は、リポジトリの `bounds.tsv` の upper 列と、まだ反映待ちの採用済み提出（D1）の最小の小さい方。
  N≤5 は確定値（下界＝上界）なので、短い提出は理論上ありえない。判定は同じ規則でよい（来たら不正な手順で落ちる）。

## 構成
1. **Cloudflare Worker**（`judge/`、TypeScript、wrangler）。同じオリジンで次を全部出す（第三者 Cookie の問題を避ける）。
   - `GET /` 提出ページ（日本語と英語の切り替え。既定はブラウザの言語）。
   - `GET /login` → GitHub の認可画面へ（state を HttpOnly Cookie に入れて照合）。`GET /callback` で code を交換、
     `GET https://api.github.com/user` で id・login・avatar を得て、セッション Cookie を発行。`POST /logout`。
   - `GET /api/me`、`GET /api/records`（色数ごとの今の上界と、採用された提出の履歴）、`POST /api/submit`（JSON `{n, moves}`）。
2. **GitHub App を 1 つ**使う（OAuth App は使わない）。
   - ユーザーのログイン: App の client_id/client_secret で user-to-server の認可（スコープ不要）。ユーザーのトークンは識別にだけ使い、保存しない。
   - リポジトリへの書き込み: App の秘密鍵（PKCS#8 PEM、WebCrypto の RS256 で JWT を作る）→ installation access token →
     Contents API で `records/submissions/` に 1 提出 1 ファイルの JSON を作る。コミットの作者は App の bot、
     本文の末尾に `Co-authored-by: <login> <id+login@users.noreply.github.com>`。
   - Worker が書くのは `records/submissions/` の下だけ（コードで限定する）。`bounds.tsv` などは Worker では書かない。
3. **GitHub Actions**（`.github/workflows/accept.yml`）: `records/submissions/**` への push で起動。
   - `tools/accept_submissions.py` を実行する。未処理の提出ファイルを古い順に読み、**Python の replay でもう一度判定**し、
     今の上界より短ければ `solutions/best.tsv` の N 行（moves・setting・verified・moves36）と `bounds.tsv` の upper・upper_source を書き換える。
     - setting は `提出: @<login>（YYYY-MM-DD）`、upper_source は `外部の提出: @<login>`。
     - 提出ファイルに結果（accepted / not_shorter / invalid と、処理日時、処理時の上界）を書き足す。
   - 続けて `python3 tools/verify_all.py --readme` と `python3 tools/build_site.py`、論文（docs/paper.html・paper.ja.html）の
     表の N 行の上界の数値の置き換え（`tools/record_upper.py` と同じやり方。本文の文章は変えない）。
   - `github-actions[bot]` でコミットして push。push が競合したら pull --rebase してやり直す（3 回まで）。
   - `build_site.py` の `upper_kind()` が upper_source の「外部の提出」を扱えるようにする（種類 "submission"、英語は "submitted by @login"）。
   - `permissions: contents: write` だけ。提出ファイルの中身はデータとしてだけ扱い、シェルに展開しない（Python で読む）。
4. **D1**（`judge/schema.sql`）: submissions（id, created_at, github_id, login, n, moves, length, status, reason, commit_sha）。
   レート制限: 1 人 1 時間に 20 件、1 日に 100 件。不正な手順や同じ手数の提出も記録する（status で区別）。

## 提出ページ（UI）
- ログインしていなければ「GitHub でログイン」ボタン。ログイン後はアバターと login、ログアウト。
- 色数の選択（2〜24。それぞれ今の上界・下界・差を表示）、手順の貼り付け欄。
- **ブラウザ側でも同じ replay を JavaScript で即時に行い**、手数・完成したか・どの手で不正になったかを送信前に表示する。
- 送信すると Worker が判定し、結果を表示: 「新記録（X → Y）。数分でサイトに反映されます」／「今の上界 X 以下ではありません」／「不正な手順（k 手目）」。
- 記録の表（色数ごとの今の上界と保持者、採用された提出の履歴）。公開サイト（GitHub Pages）への案内リンク。
- 今の公開サイト `docs/` の見た目（`docs/style.css`）に合わせる。ダークモード対応。外部の CDN は使わない。
- 表示名は GitHub の login だけ。メールアドレスや本名は表示も保存もしない（noreply アドレスは Co-authored-by にだけ使う）。

## 安全
- セッション Cookie: HMAC-SHA256 署名（`SESSION_SECRET`）、HttpOnly・Secure・SameSite=Lax、有効期限 7 日。中身は github_id・login・発行時刻だけ。
- `POST` は Origin ヘッダが自分のオリジンと一致するときだけ受ける（CSRF 対策）。
- OAuth の state は一度きりで、10 分で失効。
- 秘密（`GITHUB_APP_ID`、`GITHUB_APP_PRIVATE_KEY`、`GITHUB_CLIENT_ID`、`GITHUB_CLIENT_SECRET`、`GITHUB_INSTALLATION_ID`、`SESSION_SECRET`）は
  wrangler secret で渡す。リポジトリに置かない。`wrangler.toml` には REPO_OWNER・REPO_NAME・BRANCH などの公開してよい値だけ。
  例のファイル `judge/.dev.vars.example` を置き、本物の `.dev.vars` は .gitignore に入れる。
- 入力の検証は Worker と Actions の両方で行う。手順の文字列以外の自由入力は受け付けない。

## 作るファイル
- `judge/`: `package.json`、`wrangler.toml`、`tsconfig.json`、`src/`（index.ts、replay.ts、session.ts、github.ts、page.ts など）、
  `schema.sql`、`test/`、`README.md`（日本語。下の「配備の手順」を含める）、`.dev.vars.example`、`.gitignore`（node_modules、.dev.vars、.wrangler）。
- `tools/accept_submissions.py`（標準ライブラリのみ）とそのテスト（`tools/test_accept_submissions.py`、一時ディレクトリにリポジトリの写しを作って試す）。
- `.github/workflows/accept.yml`。
- `records/submissions/.gitkeep`。
- `tools/build_site.py` の upper_kind の小さな追加。`docs/index.html` などへのリンクはまだ入れない（URL が決まってから私が入れる）。

## 検証（配備はしない）
- `npm install`（judge/ の中だけ）→ `npm test`（vitest）。replay は Python の `tools/verify_all.py` の replay と、
  `solutions/best.tsv` の全行・わざと壊した手順で結果が一致することを確かめる（Python を子プロセスで呼んで突き合わせてよい）。
- セッションの署名・改ざんの検出・期限切れ、state の照合、Origin の検査、レート制限、「真に短いときだけ採用」を単体テストする。GitHub API は fetch をモックする。
- JWT（RS256）は、テスト用に生成した鍵で作り、公開鍵で検証できることを確かめる。
- `wrangler dev --local` で起動し、`/` が表示されること、ログインせずに `/api/submit` が 401 になることを curl で確かめる。
- `tools/test_accept_submissions.py` を通す。
- 結果を `judge/RESULTS.md` に書く。

## 配備の手順（README に書くだけ。実行しない）
1. GitHub App を作る（Callback URL = Worker の URL + `/callback`、Repository permissions: Contents = Read and write、Webhook は無効）。
   秘密鍵を作って `openssl pkcs8 -topk8 -nocrypt -in key.pem -out key-pkcs8.pem` で変換。App をこのリポジトリにだけインストール。
2. `wrangler d1 create colorsort-judge` → `wrangler.toml` に id → `wrangler d1 execute ... --file schema.sql --remote`。
3. `wrangler secret put` で秘密を入れる。`wrangler deploy`。
4. リポジトリの Settings → Actions → Workflow permissions を確認。

## 制約
- 変えてよいのは `judge/`、`tools/accept_submissions.py`、`tools/test_accept_submissions.py`、`.github/workflows/accept.yml`、
  `records/`、`tools/build_site.py`（upper_kind の追加だけ）。
- git commit・push、wrangler deploy・login、GitHub App や Cloudflare のアカウント操作はしない。
- 動いている計算（colorlb、big、iterate_big.py、colorsat.py、別の codex 作業の beam/・lowerbound/）を止めない。
- `npm install` は judge/ の中だけ。グローバルには入れない。
