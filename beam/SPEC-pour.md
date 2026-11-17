# 注ぎ単位のビームを並列・省メモリにし、下界の枝刈りと途中からの解き直しを足す（仕様）

## 背景
- `colorsort_runs/pour/beamMN.cpp`（Fable の beamM。`-DMAXN=N` で色数ごとにビルド）が、13〜15 色で私たちのビームより大幅に短い解を出した
  （13 色 329、14 色 392、15 色 501。幅 1e4、1 スレッド）。幅 2000 → 1e4 で 15 色 515 → 501 と、幅がよく効く。
- 手法: 1 ステップ＝「注ぎ」（移動元 a の上から r 個を b へ。r は 1、上に続く同色の塊（置き先の空きまで）、入るだけ全部 の 3 通り。費用 r）。
  手数 g ごとのバケツに候補を入れ、g の小さい順にバケツを評価値で並べて上位 W 個を残し、展開する。一度出た盤面（レーンの並べ替えに依らないハッシュ）は捨てる。
  評価 h = M + a·T + b·S + d·P（beamMN.cpp の heur）、同点崩しに 0〜0.02 の乱数。詳しくはソースを読むこと。
- 弱点: 1 スレッド。残した盤面を全部メモリに持つ（W × 手数 個）ので幅 1e5 以上が苦しい。開始局面が初期配置に固定。

## 作るもの: `beam/src/bin/pour.rs`（新しいバイナリ。big.rs は別作業が触っているので変更しない）
1. **beamM と同じ探索**（手の候補・バケツ・評価・重複除去）を Rust で。`pour N W [--a 0.25 --b 0.5 --d 1] [--seed S] [--threads T]`。
   出力は 1 行目 `N=… moves=…`、2 行目に 1 手 2 文字（base36）の手順。
2. **並列化**: バケツの展開と評価を rayon で。結果は同じ seed・スレッド数なら決定的にする（候補ごとに seed と親・手から乱数を作るなど）。
3. **省メモリ**: 残した節点は（親の番号、a、b、r）だけを持ち、盤面は「まだ展開に使う範囲」（g から g+N のバケツが参照しうるもの）だけ保持して古いものを解放する。
   盤面は色 5 ビット程度に詰めてよい。重複除去の集合は 64 ビットハッシュ（今と同じ）。幅 1e5〜1e6 で N=16 が数 GB に収まることを目標にする。
4. **`--start LANES`**（`bin/big` と同じ書式。レーンを下から上へ色の base36、カンマ区切り、空は "."）: 任意の局面から開始。
5. **`--upper U`**: 下界 LB で g + LB ≥ U の候補を捨てる。LB は beamMN の M（必ず動く玉の数）を基本に、
   `lowerbound/src/main.rs` の h+D または J（`jlb`/`jlb_thr`、閾値判定版が速い）を移植または呼び出して使えるようにする（`--lb m|hd|j`）。
   下界は健全（最短手数を超えない）であること。J の定義と証明は `LOWER_BOUND_PROOF.md`。
6. **`--eval-lb`**: 評価の M を選んだ下界に置き換える（重みは同じ）。効くかを測る。
7. **`tools/pour_tail.py`**: `colorsort_runs/tail/iterate_big.py` と同じ手順で、既知の解の t 手目の局面から `pour --start` で後半を解き直し、短ければ採用して繰り返す。
   ★ 改善の出力の形は iterate_big.py に合わせる（`★ N=… 改善 X -> Y` と、best_N.txt への追記）。
8. **`tools/pour_tune.py`**: 色数ごとに (a, b, d) を乱択（またはごく小さい CMA 風）で探す。小さい幅で評価し、良い重みを大きい幅で確かめる。

## 計測（この Mac は負荷が非常に高い。`nice -n 10`、スレッド 4 以下、同時 2 本まで）
- 正しさ: 出た手順を `tools/verify_all.py` の replay で確かめる。N=12 幅 2000 で、beamM（Mac の clang 版、`colorsort_runs/pour/beamM`）と種 1〜8 の手数の分布が同程度であること
  （乱数の作り方が違うので一致はしない）。
- 速さ: 同じ幅で beamM との時間比（スレッド 1 と 4）。
- 幅の効き: N=13・15 で幅 1e4、3e4、1e5（メモリが許せば 3e5）の手数・時間・最大 RSS。
- `--upper`（今の上界: 13 色 329、14 色 392、15 色 501、16 色 613）と `--lb`・`--eval-lb` の効き。
- `pour_tail.py` を N=13 の 329 手（`colorsort_runs/pour/c13.txt`）に幅 1e4 で一部の t だけ試す。
- 結果を `beam/RESULTS-pour.md` に。改善した手順は `beam/pour-runs/` に置く（bounds.tsv・solutions/ は変えない。記録は私が行う）。

## 制約
- 変えてよいのは `beam/src/bin/pour.rs`、`tools/pour_tail.py`、`tools/pour_tune.py`、`beam/RESULTS-pour.md`、`beam/pour-runs/`。
  lowerbound の J を使うときは、必要な部分を pour.rs に写すか、lowerbound をライブラリとして参照する（lowerbound/ の既存ファイルは変えない）。
- ビルドは `CARGO_TARGET_DIR=beam/target-pour`。git commit はしない。動いている計算（colorlb、big、iterate_big.py、colorsat.py、wscan.py、別の codex 作業）を止めない。新しいソフトを入れない。
