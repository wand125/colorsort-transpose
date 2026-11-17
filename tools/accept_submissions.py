"""Revalidate submission JSON and update bounds (standard library only).

python3 tools/accept_submissions.py [--root REPOSITORY]
README/site generation is a separate workflow step.
"""
import argparse
import csv
from datetime import datetime, timezone
import io
import json
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
from verify_all import replay

ROOT = Path(__file__).resolve().parents[1]
LOGIN = re.compile(r"[A-Za-z\d](?:[A-Za-z\d-]{0,37}[A-Za-z\d])?", re.ASCII)
RESULTS = {"accepted", "not_shorter", "invalid"}


def timestamp(value):
    if not isinstance(value, str) or len(value) > 40:
        raise ValueError("created_at")
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise ValueError("created_at timezone")
    return parsed.astimezone(timezone.utc)


def read_tsv(path):
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        return reader.fieldnames, list(reader)


def write_atomic(path, text):
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(text, encoding="utf-8")
    temporary.replace(path)


def tsv_text(columns, rows):
    out = io.StringIO(newline="")
    writer = csv.DictWriter(out, fieldnames=columns, delimiter="\t", lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    return out.getvalue()


def validate(data):
    if type(data.get("n")) is not int or not 2 <= data["n"] <= 24:
        raise ValueError("n")
    if type(data.get("github_id")) is not int or not 0 < data["github_id"] <= 2**53 - 1:
        raise ValueError("github_id")
    if not isinstance(data.get("login"), str) or not LOGIN.fullmatch(data["login"]):
        raise ValueError("login")
    timestamp(data.get("created_at"))
    moves = data.get("moves")
    if not isinstance(moves, str):
        raise ValueError("moves")
    moves = moves.strip()
    if not 2 <= len(moves) <= 10000 or len(moves) % 2 or not re.fullmatch(r"[0-9a-z]+", moves):
        raise ValueError("moves format")
    if type(data.get("length")) is not int or data["length"] != len(moves) // 2:
        raise ValueError("length")
    if not replay(data["n"], moves):
        raise ValueError("replay")
    return moves


def paper_outputs(root, updates):
    outputs = []
    for name in ("paper.html", "paper.ja.html"):
        path = root / "docs" / name
        text = path.read_text(encoding="utf-8")
        # Select the upper cell, even where the lower equals the old upper.
        for n, (old, new) in updates.items():
            pattern = re.compile(r'(<tr>\s*<td class="num">' + str(n) + r'</td>\s*<td class="num">[^<]+</td>\s*<td class="num">)([^<]+)(</td>)')
            def replace(match):
                value = int(match[2].replace(",", ""))
                if value not in (old, new):
                    raise ValueError(f"{name}: unexpected upper for N={n}: {value}")
                return match[1] + (f"{new:,}" if "," in match[2] else str(new)) + match[3]
            text, hits = pattern.subn(replace, text)
            if hits != 1:
                raise ValueError(f"{name}: N={n} table row count {hits}")
        outputs.append((path, text))
    return outputs


def accept_submissions(root=ROOT, now=None):
    root = Path(root)
    processed_at = (now or datetime.now(timezone.utc)).isoformat(timespec="milliseconds").replace("+00:00", "Z")
    bound_columns, bounds = read_tsv(root / "bounds.tsv")
    best_columns, best = read_tsv(root / "solutions" / "best.tsv")
    by_n = {int(row["N"]): row for row in bounds}
    best_by_n = {int(row["N"]): row for row in best}
    submissions = []
    for path in (root / "records" / "submissions").glob("*.json"):
        if path.is_symlink():
            raise ValueError(f"symlink submission: {path.name}")
        try:
            if path.stat().st_size > 262144:
                raise ValueError("file too large")
            data = json.loads(path.read_text(encoding="utf-8"))
        except (ValueError, UnicodeError):
            data = {"processing_input_error": "malformed JSON or oversized file"}
        if not isinstance(data, dict):
            data = {"processing_input_error": "JSON object required"}
        if isinstance(data.get("result"), str) and data["result"] in RESULTS and data.get("processed_at"):
            continue
        try:
            date = timestamp(data.get("created_at"))
        except (ValueError, TypeError):
            date = datetime.max.replace(tzinfo=timezone.utc)
        sequence = data.get("sequence", 0)
        if type(sequence) is not int or sequence < 0:
            sequence = 0
        submissions.append((date, sequence, path.name, path, data))
    updates, outputs = {}, []
    counts = dict.fromkeys(("accepted", "not_shorter", "invalid"), 0)
    for _, _, _, path, data in sorted(submissions, key=lambda item: item[:3]):
        n = data.get("n")
        bound = by_n.get(n) if type(n) is int else None
        previous = int(bound["upper"]) if bound else None
        result, reason = "invalid", None
        try:
            moves = validate(data)
            if bound is None:
                raise ValueError("unknown n")
            if len(moves) // 2 >= previous:
                result = "not_shorter"
            else:
                result = "accepted"
                new = len(moves) // 2
                if n not in best_by_n:
                    row = dict.fromkeys(best_columns, "")
                    row["N"] = str(n)
                    best.append(row)
                    best_by_n[n] = row
                best_by_n[n].update(moves=str(new), setting=f"提出: @{data['login']}（{timestamp(data['created_at']).date().isoformat()}）", verified="True", moves36=moves)
                bound.update(upper=str(new), upper_source=f"外部の提出: @{data['login']}", solution="solutions/best.tsv")
                updates[n] = (updates.get(n, (previous, new))[0], new)
        except (ValueError, TypeError, KeyError) as error:
            reason = str(error)
        data.update(result=result, processed_at=processed_at, previous_upper=previous, processing_reason=reason)
        outputs.append((path, json.dumps(data, ensure_ascii=True, indent=2) + "\n"))
        counts[result] += 1
    if updates:
        generated = paper_outputs(root, updates)
        generated += [
            (root / "solutions" / "best.tsv", tsv_text(best_columns, sorted(best, key=lambda row: int(row["N"])))),
            (root / "bounds.tsv", tsv_text(bound_columns, bounds)),
        ]
        for path, text in generated:
            write_atomic(path, text)
    for path, text in outputs:
        write_atomic(path, text)
    return counts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    print(json.dumps(accept_submissions(args.root), ensure_ascii=False))


if __name__ == "__main__":
    main()
