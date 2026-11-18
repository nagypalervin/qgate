"""For each sampled duplication finding, look for its block in the base.

Prints, per finding id: the status of the reported file and of the file
it duplicates in the pull request (A added, M modified, R renamed, = not
touched), and how often the block's text, whitespace removed, occurs in
the base revision: in how many files, and the most in one file. A block
that occurs twice in one base file and is reported now is repetition
that a split turned into a copy across files.

Usage: dupcheck.py <commits.tsv> <repos dir> <sample.md>
"""
import csv
import re
import subprocess
import sys

DUP = re.compile(r"duplicates (\S+):(\d+)-(\d+) \(lines (\d+)-(\d+)")


def git(repo, *args):
    return subprocess.run(["git", "-C", repo, *args], capture_output=True, text=True).stdout


def squash(text):
    return re.sub(r"\s+", "", text)


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
        target, _, _, first, last = DUP.search(message).groups()
        commit = commits[(repo, pr)]
        r = f"{repos}/{repo}"
        status = {}
        for row in git(r, "diff", "--name-status", "-M", commit + "^", commit).splitlines():
            parts = row.split("\t")
            status[parts[-1]] = parts[0][0]
        lines = git(r, "show", f"{commit}:{path}").splitlines()[int(first) - 1:int(last)]
        block = squash("\n".join(lines))
        # Candidate base files: those containing the block's longest line.
        probe = max((l.strip() for l in lines), key=len)
        files = git(r, "grep", "-l", "-F", "-e", probe, commit + "^", "--").splitlines()
        counts = []
        for name in files:
            body = squash(git(r, "show", name))
            counts.append(body.count(block))
        counts = [c for c in counts if c]
        print(f"{ident:5} {status.get(path, '='):1} -> {status.get(target, '='):1}  "
              f"base: {len(counts)} files, max {max(counts, default=0)} in one  {path} -> {target}")


if __name__ == "__main__":
    main()
