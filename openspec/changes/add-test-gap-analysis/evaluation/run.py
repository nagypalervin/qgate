"""Run qgate (this branch, and 0.6.0) over the last 30 non-merge commits
of a repository that touch the given code. Usage:
run.py <repo dir> <out dir> <qgate-new> <qgate-060> <pathspec>...
"""
import json, os, subprocess, sys, time

repo, out, new, old = sys.argv[1:5]
specs = sys.argv[5:]
CONFIG = '[files]\nexclude = ["**/Migrations/**"]\n'

def git(*args):
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True, text=True).stdout

branch = git("rev-parse", "--abbrev-ref", "HEAD").strip()
commits = []
for c in git("log", "--no-merges", "--format=%H", "-n", "200", "--", *specs).split():
    # The parent must be in the shallow history.
    if subprocess.run(["git", "-C", repo, "cat-file", "-e", c + "^"], capture_output=True).returncode == 0:
        commits.append(c)
    if len(commits) == 30:
        break

results = []
for c in commits:
    git("checkout", "-q", "--detach", "--force", c)
    with open(os.path.join(repo, "qgate.toml"), "w") as f:
        f.write(CONFIG)
    row = {"commit": c, "subject": git("log", "-1", "--format=%s", c).strip(),
           "files": git("diff", "--name-only", c + "^", c).split()}
    for name, binary in [("new", new), ("old", old)]:
        start = time.monotonic()
        p = subprocess.run([binary, "check", "--base", c + "^", "--worktree", "--format", "json", "--fail-on", "never"],
                           cwd=repo, capture_output=True, text=True)
        row[name + "_time"] = time.monotonic() - start
        if p.returncode != 0:
            row[name + "_error"] = p.stderr
            continue
        row[name] = json.loads(p.stdout)["findings"]
    os.remove(os.path.join(repo, "qgate.toml"))
    results.append(row)
git("checkout", "-q", "--force", branch)
with open(os.path.join(out, os.path.basename(repo.rstrip("/")) + ".json"), "w") as f:
    json.dump(results, f, indent=1)
print(os.path.basename(repo), len(results), "commits")
