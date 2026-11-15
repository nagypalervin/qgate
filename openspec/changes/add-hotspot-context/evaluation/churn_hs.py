"""The churn half of the hotspot definition, per family, at the newest
evaluated commit of a repository: files with at least 5 non-merge
commits in the 182 days before it, with fewer than 10% of the family's
files above. Tests and migrations are left out by name, approximately
as qgate does. Without qgate's complexity this is an upper bound on the
hotspots. Usage: churn_hs.py <repo> <pathspec>..."""
import re, subprocess, sys
from collections import Counter

repo, specs = sys.argv[1], sys.argv[2:]
TEST = re.compile(r"(\.(spec|test)\.(ts|mts|cts|tsx)$)|(/(spec|test|tests|__tests__|e2e)/)|(^(test|tests|e2e)/)"
                  r"|(\.[A-Za-z]*Tests?/)|(Migrations/)")

def git(*args):
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True, text=True).stdout

head = git("log", "--no-merges", "--format=%H", "-n", "1", "--", *specs).split()[0]
ct = int(git("show", "-s", "--format=%ct", head))
tree = [p for p in git("ls-tree", "-r", "--name-only", head).split("\n") if p]
churn = Counter(p for p in git("log", "--no-merges", "--name-only", "--format=",
                               f"--since=@{ct - 182 * 86400}", head).split("\n") if p)
for family, ext in [("TypeScript", (".ts", ".mts", ".cts", ".tsx")), ("C#", (".cs",))]:
    files = [p for p in tree if p.endswith(ext) and not TEST.search(p) and not p.endswith(".d.ts")]
    if not files:
        continue
    counts = [churn[p] for p in files]
    qualified = [p for p in files if churn[p] >= 5 and sum(c > churn[p] for c in counts) * 100 < 10 * len(files)]
    print(f"{family}: {len(files)} ranked files, highest churn {max(counts)}, churn-qualified {len(qualified)}")
