"""For each sampled duplication finding, count the lines of the reported
block and of its target that the pull request added or changed.

A block with only a line or two changed, whose target changed the same
way, was a copy before the pull request: the edit, not the copy, is new.

Usage: dupedit.py <commits.tsv> <repos dir> <sample.md>
"""
import csv
import re
import subprocess
import sys

DUP = re.compile(r"duplicates (\S+):(\d+)-(\d+) \(lines (\d+)-(\d+)")
HUNK = re.compile(r"^@@ -\S+ \+(\d+)(?:,(\d+))? @@")


def added(repo, commit, path):
    """Line numbers in the head file that the commit added or changed."""
    out = subprocess.run(["git", "-C", repo, "diff", "-U0", "-M", commit + "^", commit, "--", path],
                         capture_output=True, text=True).stdout
    lines = set()
    for line in out.splitlines():
        m = HUNK.match(line)
        if m:
            start, count = int(m.group(1)), int(m.group(2) or 1)
            lines.update(range(start, start + count))
    return lines


def main():
    listing, repos, sample = sys.argv[1:4]
    with open(listing) as f:
        commits = {(r["repo"], r["pr"]): r["commit"] for r in csv.DictReader(f, delimiter="\t")}
    for line in open(sample):
        cells = [c.strip() for c in line.split("|")]
        if len(cells) < 10 or cells[6] != "duplication/duplicate-block":
            continue
        ident, repo, pr, location, message = cells[1], cells[2], cells[3], cells[7], cells[8]
        path = location.strip("`").rsplit(":", 1)[0]
        target, t1, t2, b1, b2 = DUP.search(message).groups()
        commit = commits[(repo, pr)]
        r = f"{repos}/{repo}"
        own = added(r, commit, path) & set(range(int(b1), int(b2) + 1))
        other = added(r, commit, target) & set(range(int(t1), int(t2) + 1))
        print(f"{ident:5} block {len(own):3}/{int(b2) - int(b1) + 1:3} new, "
              f"target {len(other):3}/{int(t2) - int(t1) + 1:3} new")


if __name__ == "__main__":
    main()
