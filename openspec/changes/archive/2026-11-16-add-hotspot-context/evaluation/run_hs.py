"""Run qgate (this branch, and 0.7.0) over the last 30 non-merge commits
of a repository that touch the given code, and time the history read
that hotspot context adds. Usage:
run_hs.py <repo dir> <out dir> <qgate-new> <qgate-old> <pathspec>...
"""
import json, os, subprocess, sys, time

repo, out, new, old = sys.argv[1:5]
specs = sys.argv[5:]
CONFIG = '[files]\nexclude = ["**/Migrations/**"]\n'
WINDOW = 182 * 86400

def git(*args):
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True, text=True).stdout

branch = git("rev-parse", "--abbrev-ref", "HEAD").strip()
commits = git("log", "--no-merges", "--format=%H", "-n", "30", "--", *specs).split()
results = []
for c in commits:
    git("checkout", "-q", "--detach", "--force", c)
    with open(os.path.join(repo, "qgate.toml"), "w") as f:
        f.write(CONFIG)
    row = {"commit": c, "subject": git("log", "-1", "--format=%s", c).strip()}
    for name, binary in [("new", new), ("old", old)]:
        start = time.monotonic()
        p = subprocess.run([binary, "check", "--base", c + "^", "--worktree", "--format", "json", "--fail-on", "never"],
                           cwd=repo, capture_output=True, text=True)
        row[name + "_time"] = time.monotonic() - start
        row[name + "_stderr"] = p.stderr
        if p.returncode != 0:
            row[name + "_error"] = p.stderr
            continue
        row[name] = json.loads(p.stdout)["findings"]
    # The history read alone, as qgate runs it.
    ct = int(git("show", "-s", "--no-show-signature", "--format=%ct", c).strip())
    start = time.monotonic()
    subprocess.run(["git", "-C", repo, "log", "--topo-order", "--find-renames", "--name-status", "-z",
                    "--no-ext-diff", "--no-show-signature", "--format=commit %H %ct %P",
                    f"--since=@{ct - WINDOW}", c], check=True, capture_output=True)
    row["log_time"] = time.monotonic() - start
    os.remove(os.path.join(repo, "qgate.toml"))
    results.append(row)
git("checkout", "-q", "--force", branch)
with open(os.path.join(out, os.path.basename(repo.rstrip("/")) + ".json"), "w") as f:
    json.dump(results, f, indent=1)
print(os.path.basename(repo), len(results), "commits")
