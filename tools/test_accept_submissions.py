"""Acceptance tests: every mutation runs in a temporary repository copy."""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from datetime import datetime, timezone

sys.dont_write_bytecode = True
from accept_submissions import accept_submissions, read_tsv, tsv_text

SOURCE = Path(__file__).resolve().parents[1]
NOW = datetime(2026, 11, 17, tzinfo=timezone.utc)


class AcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="colorsort-judge-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for directory in ("solutions", "docs", "tools", "records/submissions"):
            (self.root / directory).mkdir(parents=True, exist_ok=True)
        for filename in ("bounds.tsv", "README.md", "README.ja.md"):
            shutil.copy2(SOURCE / filename, self.root / filename)
        for filename in (SOURCE / "solutions").glob("*.tsv"):
            shutil.copy2(filename, self.root / "solutions" / filename.name)
        for filename in (SOURCE / "docs").glob("*.html"):
            shutil.copy2(filename, self.root / "docs" / filename.name)
        for filename in ("verify_all.py", "build_site.py", "canon.py"):
            shutil.copy2(SOURCE / "tools" / filename, self.root / "tools" / filename)
        _, best = read_tsv(self.root / "solutions/best.tsv")
        self.seq = next(row["moves36"] for row in best if row["N"] == "7")
        self.moves = len(self.seq) // 2
        self.original_upper = self.moves

    def baseline(self, extra=4):
        self.original_upper = self.moves + extra
        for filename, column in (("bounds.tsv", "upper"), ("solutions/best.tsv", "moves")):
            path = self.root / filename
            columns, rows = read_tsv(path)
            for row in rows:
                if row["N"] == "7":
                    row[column] = str(self.original_upper)
                    if column == "moves":
                        row["moves36"] = "1001" * (extra // 2) + self.seq
            path.write_text(tsv_text(columns, rows))
        for name in ("paper.html", "paper.ja.html"):
            path = self.root / "docs" / name
            text = path.read_text()
            pattern = r'(<tr><td class="num">7</td><td class="num">[^<]+</td><td class="num">)[^<]+'
            path.write_text(re.sub(pattern, lambda m: m[1] + str(self.original_upper), text))

    def submission(self, name, **overrides):
        data = dict(id=name, sequence=1, created_at="2026-11-16T01:00:00.000Z", github_id=123,
                    login="alice", n=7, moves=self.seq, length=self.moves, status="accepted")
        data.update(overrides)
        path = self.root / "records/submissions" / (name + ".json")
        path.write_text(json.dumps(data))
        return path

    def result(self, path):
        return json.loads(path.read_text())

    def test_chronological_acceptance_strict_ties_and_idempotence(self):
        self.baseline()
        early = self.submission("z-first", created_at="2026-11-15T01:00:00Z", moves="1001" + self.seq, length=self.moves + 2)
        winner = self.submission("a-second", login="bob", sequence=2)
        tie = self.submission("a-third", login="charlie", sequence=3)
        invalid = self.submission("invalid", moves="00", length=1)
        self.assertEqual(accept_submissions(self.root, NOW), dict(accepted=2, not_shorter=1, invalid=1))
        self.assertEqual(self.result(early)["previous_upper"], self.moves + 4)
        self.assertEqual(self.result(winner)["previous_upper"], self.moves + 2)
        self.assertEqual(self.result(tie)["result"], "not_shorter")
        self.assertEqual(self.result(invalid)["result"], "invalid")
        _, best = read_tsv(self.root / "solutions/best.tsv")
        row = next(r for r in best if r["N"] == "7")
        self.assertEqual(row["setting"], "提出: @bob（2026-11-16）")
        self.assertEqual(row["verified"], "True")
        self.assertEqual(row["moves36"], self.seq)
        _, bounds = read_tsv(self.root / "bounds.tsv")
        row = next(r for r in bounds if r["N"] == "7")
        self.assertEqual(row["upper_source"], "外部の提出: @bob")
        snapshot = {p: p.read_bytes() for p in self.root.rglob('*') if p.is_file()}
        self.assertEqual(accept_submissions(self.root, NOW), dict.fromkeys(("accepted", "not_shorter", "invalid"), 0))
        self.assertEqual(snapshot, {p: p.read_bytes() for p in snapshot})

    def test_invalid_metadata_moves_and_declared_lengths_are_rejected(self):
        self.baseline()
        bad = [dict(n=True), dict(n=1), dict(n=25), dict(n="7"), dict(github_id=True), dict(github_id=0),
               dict(login="<script>"), dict(login="a\nb"), dict(login="alice\tforged"), dict(created_at="no date"),
               dict(created_at="2026-11-16"), dict(moves="\ud8000", length=1), dict(moves="A0", length=1), dict(moves="zz", length=1),
               dict(moves="0", length=0), dict(moves="", length=0), dict(moves="00", length=1),
               dict(moves="10", length=1), dict(moves="10"*5001, length=5001), dict(length=1),
               dict(moves="$(touch evil)", length=6), dict(n=[])]
        paths = [self.submission(str(i), **values) for i, values in enumerate(bad)]
        original = (self.root / "bounds.tsv").read_bytes()
        self.assertEqual(accept_submissions(self.root, NOW)["invalid"], len(paths))
        self.assertEqual((self.root / "bounds.tsv").read_bytes(), original)
        self.assertTrue(all(self.result(p)["result"] == "invalid" for p in paths))
        self.assertFalse((self.root / 'evil').exists())

    def test_nonshorter_and_exact_small_n_are_recorded_without_changes(self):
        equal = self.submission("equal")
        longer = self.submission("longer", moves="1001" + self.seq, length=self.moves + 2)
        small = self.submission("small", n=2, moves="102012", length=3)
        original = (self.root / "bounds.tsv").read_bytes()
        self.assertEqual(accept_submissions(self.root, NOW), dict(accepted=0, not_shorter=3, invalid=0))
        for p in (equal, longer, small): self.assertEqual(self.result(p)["result"], "not_shorter")
        self.assertEqual((self.root / "bounds.tsv").read_bytes(), original)

    def test_outer_whitespace_and_claimed_worker_status_are_not_trusted(self):
        self.baseline()
        path = self.submission("valid", status="invalid", moves=" \n" + self.seq + "\t ")
        self.assertEqual(accept_submissions(self.root, NOW)["accepted"], 1)
        self.assertEqual(self.result(path)["result"], "accepted")

    def test_malformed_json_and_nonobjects_are_annotated_invalid(self):
        directory = self.root / 'records/submissions'
        for name, raw in [('syntax','{'), ('list','[]'), ('null','null'), ('unicode',b'\xff')]:
            (directory / (name+'.json')).write_bytes(raw if isinstance(raw,bytes) else raw.encode())
        self.assertEqual(accept_submissions(self.root, NOW)['invalid'], 4)
        for p in directory.glob('*.json'): self.assertEqual(self.result(p)['result'], 'invalid')

    def test_symlinks_are_never_followed_or_overwritten(self):
        target = self.root / 'bounds.tsv'
        original = target.read_bytes()
        (self.root / 'records/submissions/link.json').symlink_to(target)
        with self.assertRaisesRegex(ValueError, 'symlink'):
            accept_submissions(self.root, NOW)
        self.assertEqual(target.read_bytes(), original)

    def test_paper_replacement_preserves_lower_bound_and_narrative(self):
        self.baseline()
        before = {name: (self.root / 'docs' / name).read_text() for name in ('paper.html', 'paper.ja.html')}
        self.submission('accepted')
        accept_submissions(self.root, NOW)
        for name, original in before.items():
            pattern = r'(<tr><td class="num">7</td><td class="num">[^<]+</td><td class="num">)[^<]+'
            expected = re.sub(pattern, lambda m: m[1]+str(self.moves), original)
            self.assertEqual((self.root / 'docs' / name).read_text(), expected)

    def test_full_verification_readme_and_site_generation_on_copy(self):
        self.baseline()
        self.submission('accepted')
        accept_submissions(self.root, NOW)
        env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1')
        for args in [('verify_all.py','--readme'), ('build_site.py',)]:
            run = subprocess.run([sys.executable, str(self.root / 'tools' / args[0]), *args[1:]], cwd=self.root, env=env, capture_output=True, text=True, timeout=120)
            self.assertEqual(run.returncode, 0, run.stdout+run.stderr)
        data = json.loads((self.root / 'docs/data/bounds.json').read_text())
        row = next(r for r in data if r['N'] == 7)
        self.assertEqual(row['upper_en'], 'submitted by @alice')
        self.assertIn('外部の提出: @alice', (self.root / 'README.ja.md').read_text())


class WorkflowTests(unittest.TestCase):
    def script(self):
        source = (SOURCE / '.github/workflows/accept.yml').read_text()
        return '\n'.join(line[10:] for line in source.split('        run: |\n',1)[1].splitlines())

    def test_definition_and_shell_syntax(self):
        source = (SOURCE / '.github/workflows/accept.yml').read_text()
        self.assertIn('permissions:\n  contents: write', source)
        self.assertNotIn('pull_request_target', source)
        self.assertNotIn('github.event.', source)
        result = subprocess.run(['/bin/bash','-n'], input=self.script(), text=True, capture_output=True)
        self.assertEqual(result.returncode,0,result.stderr)

    def test_push_retry_and_conflict_recovery_using_fake_git_only(self):
        with tempfile.TemporaryDirectory(prefix='colorsort-workflow-mock-') as temporary:
            directory = Path(temporary)
            # Records commands as data; never invokes real git.
            fake = directory/'git'
            fake.write_text('#!'+sys.executable+'\n'+r'''
import json,os,sys
from pathlib import Path
p=Path(os.environ['TRACE'])
entries=json.loads(p.read_text()) if p.exists() else []
entries.append(sys.argv[1:]);p.write_text(json.dumps(entries))
args=sys.argv[1:]
if args[:1]==['diff']: sys.exit(1)
if args[:1]==['push']:
 count=sum(e[:1]==['push'] for e in entries)
 sys.exit(1 if count <= int(os.environ['FAIL_PUSHES']) else 0)
if args[:1]==['pull']: sys.exit(int(os.environ['FAIL_REBASE']))
''')
            fake.chmod(0o755)
            fake_python = directory/'python3'
            fake_python.write_text('#!/bin/sh\nexit 0\n');fake_python.chmod(0o755)
            for failures,rebase,status,pushes in [(0,0,0,1),(1,0,0,2),(1,1,0,2),(3,1,1,3)]:
                trace=directory/'trace.json'
                if trace.exists(): trace.unlink()
                env=dict(os.environ, PATH=str(directory)+os.pathsep+os.environ['PATH'], TRACE=str(trace), FAIL_PUSHES=str(failures), FAIL_REBASE=str(rebase), TARGET_BRANCH='test-branch')
                run=subprocess.run(['/bin/bash'],input=self.script(),text=True,cwd=directory,env=env,capture_output=True,timeout=30)
                self.assertEqual(run.returncode,status,run.stderr)
                entries=json.loads(trace.read_text())
                self.assertEqual(sum(e[:1]==['push'] for e in entries),pushes)
                if failures and rebase:
                    self.assertIn(['rebase','--abort'],entries)
                    self.assertIn(['reset','--hard','FETCH_HEAD'],entries)


if __name__ == '__main__':
    unittest.main()
