"""Check each listed pull request as the Azure job would (design D1).

For each commit C of the list: check out C, place the repository's frozen
qgate.toml (untracked), run
  qgate check --base C^ --worktree --format json --fail-on never
and save the report and the wall time of that one command.

Usage: run.py <qgate> <commits.tsv> <repos dir> <config dir> <out dir> [commit...]
With commits given, only those are run (to repeat a run).
Writes <out dir>/<repo>/<pr>.json and appends to <out dir>/runs.tsv.
"""
import csv
import os
import shutil
import subprocess
import sys
import time


def git(repo, *args):
    return subprocess.run(["git", "-C", repo, *args], check=True,
                          capture_output=True, text=True).stdout


def main():
    qgate, listing, repos, configs, out = sys.argv[1:6]
    only = set(sys.argv[6:])
    with open(listing) as f:
        rows = [r for r in csv.DictReader(f, delimiter="\t") if not only or r["commit"] in only]
    runs = os.path.join(out, "runs.tsv")
    new = not os.path.exists(runs)
    with open(runs, "a") as log:
        if new:
            log.write("repo\tcommit\tpr\tseconds\texit\n")
        for r in rows:
            repo = os.path.join(repos, r["repo"])
            config = os.path.join(repo, "qgate.toml")
            git(repo, "checkout", "-q", "--detach", "--force", r["commit"])
            shutil.copyfile(os.path.join(configs, r["repo"] + ".toml"), config)
            start = time.monotonic()
            p = subprocess.run([qgate, "check", "--base", r["commit"] + "^", "--worktree",
                                "--format", "json", "--fail-on", "never"],
                               cwd=repo, capture_output=True, text=True)
            seconds = time.monotonic() - start
            os.remove(config)
            os.makedirs(os.path.join(out, r["repo"]), exist_ok=True)
            with open(os.path.join(out, r["repo"], r["pr"] + ".json"), "w") as f:
                f.write(p.stdout)
            if p.stderr:
                with open(os.path.join(out, r["repo"], r["pr"] + ".stderr"), "w") as f:
                    f.write(p.stderr)
            log.write(f"{r['repo']}\t{r['commit']}\t{r['pr']}\t{seconds:.2f}\t{p.returncode}\n")
            log.flush()
            print(r["repo"], r["pr"], f"{seconds:.1f}s", "exit", p.returncode, flush=True)


if __name__ == "__main__":
    main()
