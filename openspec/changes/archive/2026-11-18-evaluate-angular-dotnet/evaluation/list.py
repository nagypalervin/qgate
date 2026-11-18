"""List the last 50 pull requests of each repository's main branch.

A pull request is a first-parent commit of the branch:
- on GitHub, with one parent and a `(#N)` at the end of its subject: the
  three repositories squash pull requests, and GitHub appends the number;
- on Azure Repos (the pilot, design "Pilot plan"), with the subject
  `Merged PR N: ...` that Azure Repos writes, squashed (one parent) or
  merged (two): the first parent, `C^`, is the target before the pull
  request in both cases.
Other commits on the branch (direct pushes, such as version bumps) are
counted and left out.

Usage: list.py <out.tsv> <repo dir>...
Prints each repository's head commit and how many commits it skipped.
"""
import re
import subprocess
import sys

COUNT = 50
BOTS = ("[bot]", "renovate", "dependabot")
GITHUB = re.compile(r"\(#(\d+)\)\s*$")
AZURE = re.compile(r"^Merged PR (\d+):")


def git(repo, *args):
    return subprocess.run(["git", "-C", repo, *args], check=True,
                          capture_output=True, text=True).stdout


def main():
    out, repos = sys.argv[1], sys.argv[2:]
    rows = []
    for repo in repos:
        name = repo.rstrip("/").split("/")[-1]
        branch = git(repo, "symbolic-ref", "--short", "refs/remotes/origin/HEAD").strip()
        head = git(repo, "rev-parse", branch).strip()
        log = git(repo, "log", "--first-parent", "--format=%H%x09%P%x09%an%x09%ae%x09%s", branch)
        taken, skipped = [], 0
        for line in log.splitlines():
            commit, parents, author, email, subject = line.split("\t", 4)
            parent_count = len(parents.split())
            github, azure = GITHUB.search(subject), AZURE.search(subject)
            pr = github if parent_count == 1 else None
            pr = pr or (azure if parent_count in (1, 2) else None)
            if not pr:
                skipped += 1
                continue
            who = (author + " " + email).lower()
            bot = any(b in who for b in BOTS)
            taken.append((name, commit, pr.group(1), "bot" if bot else "human", author, subject))
            if len(taken) == COUNT:
                break
        rows += taken
        print(f"{name}: {branch} at {head}, {len(taken)} pull requests, {skipped} other commits skipped")
    with open(out, "w") as f:
        f.write("repo\tcommit\tpr\tkind\tauthor\tsubject\n")
        for row in rows:
            f.write("\t".join(row) + "\n")


if __name__ == "__main__":
    main()
