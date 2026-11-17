"""Summarize the saved reports per repository (design D4).

A pull request is commented when its report has at least one finding,
and would fail when it has at least one Error.

Usage: summary.py <commits.tsv> <out dir>
"""
import collections
import csv
import json
import os
import statistics
import sys


def main():
    listing, out = sys.argv[1:3]
    with open(listing) as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    with open(os.path.join(out, "runs.tsv")) as f:
        # The last run of a commit wins, so a repeated run replaces nothing.
        seconds = {(r["repo"], r["pr"]): float(r["seconds"]) for r in csv.DictReader(f, delimiter="\t")}
    for repo in dict.fromkeys(r["repo"] for r in rows):
        prs = [r for r in rows if r["repo"] == repo]
        count = collections.Counter()
        rules = collections.Counter()
        alone = collections.Counter()
        prs_by_rule = collections.Counter()
        times = []
        for r in prs:
            with open(os.path.join(out, repo, r["pr"] + ".json")) as f:
                findings = json.load(f)["findings"]
            times.append(seconds[(repo, r["pr"])])
            for kind in ("all", r["kind"]):
                count[kind, "prs"] += 1
                count[kind, "commented"] += bool(findings)
                count[kind, "fail"] += any(x["severity"] == "error" for x in findings)
            for x in findings:
                rules[x["rule"], x["severity"]] += 1
            fired = {x["rule"] for x in findings}
            for rule in fired:
                prs_by_rule[rule] += 1
            if len(fired) == 1:
                alone[fired.pop()] += 1
        print(f"## {repo}")
        for kind in ("all", "human", "bot"):
            n = count[kind, "prs"]
            if n:
                print(f"{kind:6} {n:3} pull requests, {count[kind, 'commented']:3} commented"
                      f" ({count[kind, 'commented'] / n:.0%}), {count[kind, 'fail']:3} would fail")
        for (rule, severity), n in sorted(rules.items()):
            print(f"  {n:4}  {severity:8} {rule}")
        # Which rules a comment depends on: pull requests a rule appears
        # in, and those it alone makes commented.
        for rule, n in sorted(prs_by_rule.items()):
            print(f"  in {n:3} pull requests, the only rule in {alone[rule]:3}: {rule}")
        print(f"time: median {statistics.median(times):.1f} s, slowest {max(times):.1f} s")
        print()


if __name__ == "__main__":
    main()
