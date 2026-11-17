# 色そろえパズルのオンラインジャッジ

GitHub App でログインし、2〜24 色の手順を提出する Cloudflare Worker です。ブラウザと Worker が同じ replay 関数を使い、Actions が既存の Python replay でもう一度検証します。`wrangler.toml` の `keep_names = false` は、ブラウザへ渡す関数にバンドル内の補助関数への参照を混入させないために必要です。

## ローカルでの確認

Node.js 22 以降、Python 3.10 以降を用意し、リポジトリの根から実行します。npm の操作は judge 内だけで行います。

```sh
cd judge
npm install
npm run types
npm run typecheck
npm test
npx wrangler d1 execute colorsort-judge --local --file schema.sql
npx wrangler dev --local --port 18787 --inspector-port 18788
```

別のターミナルから確認します。

```sh
curl -i http://127.0.0.1:18787/
curl -i http://127.0.0.1:18787/api/submit \
  -H 'Origin: http://127.0.0.1:18787' \
  -H 'Content-Type: application/json' \
  --data '{"n":2,"moves":"102012"}'
```

前者は HTML と 200、後者は未ログインのため 401 になります。Origin のない POST は 403 です。セッション Cookie は本番と同じ Secure 属性を持ちます。ローカルの curl 確認にログインや秘密情報は必要ありません。

Python 側はリポジトリの根で実行します。

```sh
PYTHONDONTWRITEBYTECODE=1 python3 tools/test_accept_submissions.py -v
```

テストは必要なファイルを一時ディレクトリにコピーし、その中だけで TSV・README・サイトを更新します。Actions の競合再試行テストも偽の git コマンドだけを使い、実際の commit・push は行いません。`tools/accept_submissions.py` を作業中のリポジトリで直接実行すると、未処理の提出がある場合に記録が更新されるため、検証にはこのテストを使ってください。

`npm test` は Workers 実行環境とローカル D1 を使います。`solutions/best.tsv` の全行と各行の破損例を、ホストの Python と照合します。外部 fetch はモックし、テスト用 RSA 鍵はその場で生成します。本物の秘密や GitHub API 書き込みは不要です。

ブラウザ操作の追加確認は、ローカル Worker を起動したまま `npm run test:browser` で行えます。Playwright が独立した空のヘッドレス環境を作ります。macOS では既存の Google Chrome を使い、他の環境では `CHROME_PATH` で実行ファイルを指定できます。アカウント・提出 API はモックされ、実提出はしません。

この環境でサンドボックス内の macOS タイムゾーン初期化が待機する場合は、端末で通常実行してください。Wrangler のログ保存先は `WRANGLER_LOG_PATH=.wrangler/logs` で judge 内に指定できます。

## データと判定

- `POST /api/submit` は署名済みセッションと同一 Origin を要求し、JSON の `n` と `moves` 以外の入力フィールドを受け付けません。
- 前後の空白を除き、1〜5000 手・偶数文字・base36 小文字を検査します。実行不能な手は最初の手番号を返します。未完成も不正な提出として記録します。
- D1 の単一 INSERT 文で、1 人あたり直近 1 時間 20 件・24 時間 100 件の上限と採否を判定します。実行不能・未完成・同手数も件数に含みます。壊れた JSON や不正な型など、提出として読み取れないリクエストは 400 です。
- 現在の上界は、毎回取得するリポジトリの bounds.tsv と D1 の採用済み提出の最小値です。等しい手数は採用しません。公開 API は login だけを表示し、個人の氏名・メール・ユーザートークンを保存しません。
- セッション内容は GitHub id・login・発行時刻だけで、HMAC-SHA256、7 日間、HttpOnly / Secure / SameSite=Lax です。アバター URL は GitHub id から作ります。OAuth state は Cookie と D1 を照合して一度だけ使え、10 分で失効します。
- 採否を D1 に保存したあと、提出 JSON を `records/submissions/<UUID>.json` に順番に作ります。GitHub App の installation token で書き込み、author / committer を指定しないため App の bot が作者になります。末尾に指定形式の Co-authored-by を付けます。
- `commit_sha IS NULL` の行は未送信として保持し、リクエスト後と 5 分ごとの Cron で古い順に最大 10 件ずつ再送します。短期リースで並列送信を防ぎます。失敗した先頭を追い越さず、応答だけ失われた成功は既存ファイルの内容を照合して回復します。Worker は既存ファイルや bounds.tsv を更新しません。
- 同一ミリ秒の提出は D1 の rowid を `sequence` として JSON に渡し、Actions の日付順の補助キーにします。Actions は Worker の status・length の自己申告を信用せず、入力検査と replay を再実行します。
- Actions は更新結果を `result`、`processed_at`、`previous_upper`、`processing_reason` に追記します。処理済みは再処理しません。論文では N 行の上界セルだけを置換し、本文を変えません。続く既存 build_site.py は表と公開データを再生成します。
- Actions は対象ブランチごとに直列実行し、push 競合時は pull --rebase で追従して全判定を再実行します。生成物が競合した場合は使い捨ての Actions checkout を最新状態へ戻して再計算します。push は最大 3 回です。

GitHub が停止している間は D1 の記録が先に表示されます。再送待ちが多い場合や Actions が失敗した場合、公開サイトへの反映は遅れます。Cron のエラーはユーザー入力・トークンを含めない固定メッセージで記録します。`commit_sha IS NULL` の件数と Actions の結果を運用時に確認してください。

## 配備の手順（この実装作業では実行しない）

1. GitHub App を 1 つ作ります。Callback URL は `https://<Worker の URL>/callback`、Repository permissions は **Contents: Read and write**、Webhook は無効にします。OAuth App は使わず、この App をこのリポジトリだけにインストールします。ユーザー認可には scope を付けません。秘密鍵を作り、PKCS#8 に変換します。

   ```sh
   openssl pkcs8 -topk8 -nocrypt -in key.pem -out key-pkcs8.pem
   ```

2. `judge/` で D1 を作り、返された id を wrangler.toml の全ゼロの仮 id と置き換えます。REPO_OWNER・REPO_NAME・BRANCH・PUBLIC_SITE_URL も確認します。

   ```sh
   npx wrangler d1 create colorsort-judge
   npx wrangler d1 execute colorsort-judge --file schema.sql --remote
   ```

3. 以下の 6 個を Wrangler secret に入れます。秘密をコマンド引数や設定ファイルに直接書かず、対話入力を使います。秘密鍵だけはファイルから入力します。SESSION_SECRET は 32 バイト以上の十分長い乱数にしてください。

   ```sh
   npx wrangler secret put GITHUB_APP_ID
   npx wrangler secret put GITHUB_APP_PRIVATE_KEY < key-pkcs8.pem
   npx wrangler secret put GITHUB_CLIENT_ID
   npx wrangler secret put GITHUB_CLIENT_SECRET
   npx wrangler secret put GITHUB_INSTALLATION_ID
   npx wrangler secret put SESSION_SECRET
   npx wrangler deploy
   ```

   ローカルで秘密が必要な場合だけ `.dev.vars.example` を参考に `.dev.vars` を作ります。`.dev.vars`・node_modules・.wrangler は .gitignore の対象です。本物の秘密や秘密鍵をリポジトリに置かないでください。

4. リポジトリの Settings → Actions → Workflow permissions で書き込みが許可されることと、対象ブランチの保護規則を確認します。ワークフローの権限指定は `contents: write` のみです。GitHub Pages は生成された docs/ を公開する設定を確認してください。このワークフローには配備や Pages の設定変更は含みません。

公開サイトへの提出ページのリンクは、URL が決まった後に追加します。現時点では docs/ にリンクを追加していません。

## 参照した公式資料

- [Cloudflare D1 の batch・セッション](https://developers.cloudflare.com/d1/worker-api/d1-database/)
- [Workers の Vitest 統合](https://developers.cloudflare.com/workers/testing/vitest-integration/write-your-first-test/)
- [Wrangler 設定](https://developers.cloudflare.com/workers/wrangler/configuration/)
- [GitHub App のユーザー認可](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)
- [GitHub App の JWT](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-json-web-token-jwt-for-a-github-app)
- [GitHub Contents API](https://docs.github.com/en/rest/repos/contents#create-or-update-file-contents)
