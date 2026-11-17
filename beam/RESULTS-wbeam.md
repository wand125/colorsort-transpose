# wbeam 実装・計測結果

2026-11-17（JST）。`beam/SPEC-wbeam.md` に従って実装し、以下の窓を計測した。

## 実装

- `beam/src/bin/wbeam.rs`: `wbeam N --from LANES --to LANES --max L --width W [--threads T]`。LANES は下から上の色を base36 で記したカンマ区切り、空は `.`。N=1..24、既定1スレッド、最大4スレッド。L−1手以内を探索する。
- 同一レーンを多重度込みで相殺し、残りの `高さ−共通の底の長さ` の割り当て最小値を整数ハンガリー法で計算する。全玉数が同じなので、これは少なくとも一度取り出す必要のある玉数の下界。相殺は共通接頭辞長の交換不等式により最適割り当てを保つ。行ごとの最小値だけで予算超過が分かる候補はハンガリー法の前に捨てる。
- `g+h > L−1` を除外。層ごとの `g+λh`（λ=1）で選抜し、同点は正規化局面のハッシュ順。レーンをソートした局面本体で重複を除去する。ハッシュはグループ化だけに使い、衝突時も局面の完全一致で判定する。直近2層への戻りも除く。
- 親と正規化局面上の手を保存し、復元時に実レーン番号へ対応させる。出力は1行のJSON。`status`, `moves`, `length`, `permutation`, `h0`, `depth`, `truncated`, `generated`, `evaluated`, `peak_candidates`, `seconds`, `threads` を含む。`permutation[元のtoレーン番号] = 実際の終点レーン番号`。終点の完全一致も確認してから出力する。
- 最初に到達した層の経路を出す。幅切り後の `FOUND` はこのビームが発見した最短の深さであり、大域的な最短性は保証しない。`NOT_FOUND` も `truncated=true` なら短縮不能の証明ではない。
- `tools/wscan.py`: 指定の `--len`, `--step`, `--width`, `--jobs`, `--from` に対応。J=1..2、各子2スレッド、ドライバーと子の nice を10以上にする。改善ごとに `★ 改善 X -> Y` と全体手順を表示する。区間終点の置換を確認し、後続手に適用して `tools/verify_all.py.replay()` で完成を再確認する。
- 並列窓は版番号を持つ。改善後、実行中の旧版の計算は完了まで待ち、その結果を捨てて同じtを新しい手順で再計算する。未実行の窓も最新の手順を使う。窓の採用順は終了順に依存しうる。
- Cargo の `src/bin/` 自動検出を使うため `beam/Cargo.toml` の変更は不要だった。

## ビルド・再現

リポジトリの根で実行する。既存ツールのみ、オフラインビルド。

```sh
CARGO_TARGET_DIR=beam/target-wbeam RUSTUP_TOOLCHAIN=1.98.0-aarch64-apple-darwin nice -n 10 cargo build --manifest-path beam/Cargo.toml --release --bin wbeam --offline -j 2
CARGO_TARGET_DIR=beam/target-wbeam RUSTUP_TOOLCHAIN=1.98.0-aarch64-apple-darwin RAYON_NUM_THREADS=4 nice -n 10 cargo test --manifest-path beam/Cargo.toml --release --bin wbeam --offline -j 2 -- --test-threads=1
PYTHONDONTWRITEBYTECODE=1 nice -n 10 python3 beam/wbeam-runs/validate.py
PYTHONDONTWRITEBYTECODE=1 nice -n 10 python3 beam/wbeam-runs/measure.py
```

`measure.py` は保存済みの入力スナップショットと個別JSONを再利用し、入力ハッシュを照合する。新規計測には別名で原記録を保全してから当該JSONを除くこと。各JSONの `command` に局面文字列を含む実行引数を保存した。`measurements.json` が集計、各窓の `.stdout`, `.stderr`, `.time`, `.json` が原記録。入力のスナップショットも保存した。

## 正しさ

- Rust の2テストが成功。30種類の6×6コスト行列でハンガリー法と全720置換の最小値を照合。N=2の全到達可能な正規化局面の各組について、全探索距離に対する下界の健全性、十分な幅での最短到達・復元、1手短い制限での未到達を照合した。`wbeam-runs/rust-tests.log`。
- N=6の既知55手に、t=10と30で「合法な1手とその逆」の回り道を挿入して59手にした。L=8、step=1、幅10000、J=2、from=7で **59 → 57 → 55**。各改善後の全体replayが成功。旧版結果の破棄・再計算も実際に起きた。`n6-detour-input.txt`, `n6-restored.txt`, `validation.log`。
- 置換した重複レーンへの0手到達、不正な置換の拒否、不正なCLI局面入力も確認。実際の `wscan.py` CLIも全指定引数を通して実行し、step=8・J=1で59→55手、全体replayを確認（`cli-validation.log`）。
- 置換方向の追加検証として、完成局面に回り道と後続手を足したN=6・79手を作成。恒等でも対合でもない3レーン巡回の置換を返す窓を0手へ置換し、後続手も付け替えて79→67→55手となることを全体replayで確認（`permutation-check.log`, `permutation_check.py`, `n6-permutation-input.txt`, `n6-permutation-restored.txt`）。
- N=24の満杯レーンで1手の移動と置換付き復元を確認。さらに一致レーンがない25×25割り当てで、手計算の下界574と一致（`boundary-check.log`, 再現は `boundary_check.py`）。
- `sat/colorsat.py` の `solve(..., target=..., bound=L−1)` と同一窓で照合（kissat、各60秒上限）。6件すべて確定して一致し、SATの4窓はwbeamでも6手から4手へ短縮し、全体replayが通った。

| 59手入力のt | L | SAT | wbeam | 区間手数 |
|---:|---:|---|---|---:|
| 0 | 4 | UNSAT | NOT_FOUND | — |
| 9 | 6 | SAT | FOUND | 4 |
| 10 | 6 | SAT | FOUND | 4 |
| 18 | 6 | UNSAT | NOT_FOUND | — |
| 29 | 6 | SAT | FOUND | 4 |
| 31 | 6 | SAT | FOUND | 4 |

## 計測条件

- macOS 26.2 / arm64、14コア、48 GiBメモリ。Rust 1.98.0、Rayon 1.12.0、release / opt-level=3 / LTO / codegen-units=1。
- nice=10、1プロセス2スレッド、同時2本。既存の計算への停止・優先度変更は行っていない。計測前のload averageは約120〜160、途中には約188まで上昇した（`environment.json`, `processes.txt` に取得値と nice の観測を保存）。
- 12色272手は `solutions/docs_known.tsv`、15色531手・16色632手は仕様の `colorsort_runs/sat/c15.txt`, `c16.txt`（実際の配置はリポジトリの隣の `../colorsort_runs/sat/`）。`n12-input.txt`, `n15-input.txt`, `n16-input.txt` と入力SHA-256を保存した。
- 12色は全組み合わせでt=24と96の2窓。15・16色はL=32、幅10000、t=32と128の各2窓。**全窓走査ではない**。
- 時間はPythonドライバーが測ったプロセス起動から終了までの壁時計秒。メモリはmacOS `/usr/bin/time -l` の `maximum resident set size`（bytes）をMiBへ換算。`wbeam` 内部の探索秒とtimeのuser/sys値も原記録にある。単回・高負荷下の値であり、特に短時間の窓では起動待ちが支配する。幅間の小さな時間差を性能差とみなさない。

## 窓ごとの実測

`幅切り` は候補をW件へ切ったか。`深さ` は最後に生成・評価した層（候補が空になった層も含む）。`h0 >= L` は探索前に下界で除外。

| N | 入力手数 | t | L | W | h0 | 深さ | 幅切り | 結果 | 秒/窓 | 最大RSS MiB |
|---:|---:|---:|---:|---:|---:|---:|:---:|---|---:|---:|
| 12 | 272 | 24 | 24 | 10000 | 24 | 0 | なし | NOT_FOUND | 0.071 | 1.66 |
| 12 | 272 | 96 | 24 | 10000 | 21 | 8 | なし | NOT_FOUND | 0.150 | 1.83 |
| 12 | 272 | 24 | 24 | 100000 | 24 | 0 | なし | NOT_FOUND | 0.078 | 1.67 |
| 12 | 272 | 96 | 24 | 100000 | 21 | 8 | なし | NOT_FOUND | 0.180 | 1.89 |
| 12 | 272 | 24 | 32 | 10000 | 32 | 0 | なし | NOT_FOUND | 0.074 | 1.64 |
| 12 | 272 | 96 | 32 | 10000 | 27 | 23 | なし | NOT_FOUND | 1.712 | 7.16 |
| 12 | 272 | 24 | 32 | 100000 | 32 | 0 | なし | NOT_FOUND | 0.060 | 1.66 |
| 12 | 272 | 96 | 32 | 100000 | 27 | 23 | なし | NOT_FOUND | 1.663 | 7.06 |
| 12 | 272 | 24 | 48 | 10000 | 44 | 27 | なし | NOT_FOUND | 2.654 | 3.33 |
| 12 | 272 | 96 | 48 | 10000 | 40 | 25 | あり | NOT_FOUND | 28.217 | 72.41 |
| 12 | 272 | 24 | 48 | 100000 | 44 | 27 | なし | NOT_FOUND | 2.075 | 3.67 |
| 12 | 272 | 96 | 48 | 100000 | 40 | 25 | あり | NOT_FOUND | 187.824 | 281.03 |
| 12 | 272 | 24 | 64 | 10000 | 59 | 54 | あり | NOT_FOUND | 31.248 | 15.53 |
| 12 | 272 | 96 | 64 | 10000 | 48 | 47 | あり | NOT_FOUND | 115.216 | 122.94 |
| 12 | 272 | 24 | 64 | 100000 | 59 | 54 | なし | NOT_FOUND | 30.713 | 17.06 |
| 12 | 272 | 96 | 64 | 100000 | 48 | 47 | あり | NOT_FOUND | 1518.803 | 508.70 |
| 15 | 531 | 32 | 32 | 10000 | 32 | 0 | なし | NOT_FOUND | 0.080 | 1.64 |
| 15 | 531 | 128 | 32 | 10000 | 32 | 0 | なし | NOT_FOUND | 0.077 | 1.66 |
| 16 | 632 | 32 | 32 | 10000 | 32 | 0 | なし | NOT_FOUND | 0.031 | 1.64 |
| 16 | 632 | 128 | 32 | 10000 | 28 | 10 | なし | NOT_FOUND | 8.848 | 11.70 |

計測完了: 20/20窓。既知解からの改善: 0件。

幅切りあり: 5窓。下界だけで即時除外: 7窓。

今回の窓と幅では、12色272手・15色531手・16色632手の既知解の改善を発見できなかった。幅切りのある窓や未走査の窓の短縮可能性は未確定。N=6の回復用手順は `wbeam-runs/n6-restored.txt` に保存した。

## 計測から分かったこと

t=96では、上限L−1と初期下界h0の差がL=24/32/48/64でそれぞれ2/4/7/15手となり、長い窓で候補数と時間が増えた。L=64・幅10万の窓は約1.73億候補を生成し、約25.3分、最大RSS約509 MiBだった。幅を増やしても今回の窓で改善は出なかったが、幅切りのある探索結果から短縮不能とは判断できない。

最終的な20窓の対応・入力とソースのハッシュ・保存手順のreplay・検証ログの整合は `wbeam-runs/audit.json` に記録。再確認は `PYTHONDONTWRITEBYTECODE=1 python3 beam/wbeam-runs/audit.py`。

## 変更範囲

ソース変更は `beam/src/bin/wbeam.rs` と `tools/wscan.py`。結果・検証コード・入力・ログは `beam/RESULTS-wbeam.md` と `beam/wbeam-runs/`。ビルド生成物は指定の `beam/target-wbeam`。`beam/src/bin/big.rs`、bounds.tsv、solutions/ は変更せず、git commit も行っていない。作業ツリーにあった他作業の変更はそのまま残した。
