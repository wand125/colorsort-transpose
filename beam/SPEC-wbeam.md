# 長い区間をビームで解き直す `wbeam`（仕様）

## 背景
- 色そろえパズル（N 色、レーン N+1 本、容量 N。初期はレーン 0 が空、1..N が下から 1..N。1 手 = 2 文字の base36 で移動元・移動先。
  完成 = どのレーンも空か同色 N 個）。再生の規則は `tools/verify_all.py` の `replay()`。
- 既知の解の改善は、(a) 途中から完成までをビームで解き直す（`colorsort_runs/tail/iterate_big.py` → `bin/big --start`）、
  (b) 12〜16 手の区間を SAT で解き直す（`sat/colorsat.py window`、行き先はレーンの付け替えを許して一致）でやってきた。
- 12 色 272 手（`solutions/docs_known.tsv`）は (a) でも、長さ 8〜16 の区間の厳密な解き直し（`tools/window.py`）でも縮まない。
  もっと長い区間（24〜80 手）の組み替えが要る。SAT はその長さに届かない。

## 作るもの
1. `beam/src/bin/wbeam.rs`（新しいバイナリ。`big.rs` は別の作業が変更中なので**触らない**。必要な部品は複製してよい）。
   - `wbeam N --from LANES --to LANES --max L --width W [--threads T]`
   - 局面 from から、局面 to（**レーンの並べ替えを許して一致**＝レーンの中身の多重集合が同じ）へ、L−1 手以内で行く道をビームで探す。
     見つかった最短の手順と、to のレーン番号への対応（置換）を出す。見つからなければそう出す。
   - 評価: g + λ·h。h は to までの下界。`tools/window.py` の下界（同じレーンを打ち消したあと、レーンどうしの割り当てで
     Σ(高さ − 共通の底の長さ) の最小）を基本にする。割り当ては N+1 ≤ 25 なのでハンガリー法で。より強い健全な下界があれば使ってよい。
     g + h > L−1 の状態は捨てる（h が下界なので健全）。
   - 重複除去はレーンの並べ替えに対して正規化した鍵で。
2. `tools/wscan.py`: 既知の解に対して窓を滑らせる。
   - `wscan.py N MOVES --len L --step S --width W [--jobs J] [--from T]`
   - t = from, from+S, … について、局面 t から局面 t+L への区間を wbeam で解き直し、短くなったら採用（後ろの手のレーン番号を置換で付け替え）して続ける。
   - 改善のたびに `★ 改善 X -> Y` と全体の手順（replay で確認済み）を出す。出力の形は `sat/colorsat.py window --scan` に合わせる。
   - J 並列で別々の t を同時に解き、改善が出たらその結果で手順を更新して、まだ解いていない t は新しい手順で解き直す。

## 計測
- 正しさ: N=6（55 手の最短解に、わざと回り道を挟んだもの）で元に戻ること。SAT の window と結果が矛盾しないこと（短い L で、SAT で縮む区間は wbeam でも縮むか）。
- 12 色 272 手で、L=24・32・48・64、幅 1e4〜1e5 を試す。改善の有無、1 窓あたりの時間、メモリ。
- 15 色（`colorsort_runs/sat/c15.txt`、531 手）と 16 色（`colorsort_runs/sat/c16.txt`、632 手）でも L=32 で一部の窓を試す。
- この Mac は負荷が非常に高い（14 コアで load 100 前後）。計測は `nice -n 10`、スレッド 4 以下、同時 2 本まで。
- 結果を `beam/RESULTS-wbeam.md` に書く。改善が出た手順は `beam/wbeam-runs/` に置く（bounds.tsv・solutions/ は変えない。記録は私が行う）。

## 制約
- 変えてよいのは `beam/src/bin/wbeam.rs`、`beam/Cargo.toml`（bin の追加だけ）、`tools/wscan.py`、`beam/RESULTS-wbeam.md`、`beam/wbeam-runs/`。
- ビルドは `CARGO_TARGET_DIR=beam/target-wbeam`（ほかの作業のビルド成果物を上書きしない）。
- git commit はしない。動いている計算（colorlb、big、iterate_big.py、colorsat.py、別の codex 作業）を止めない。新しいソフトを入れない。
