"""Merge judgements.tsv into the sample table and count the classes.

Fails if a sampled finding has no judgement, or a judgement no finding.

Usage: judge.py <sample.md> <judgements.tsv>
Prints the judged table, then the counts per class and severity.
"""
import collections
import csv
import sys

CLASSES = ("useful", "defensible", "noise")


def main():
    sample, judgements = sys.argv[1:3]
    with open(judgements) as f:
        judged = {r["id"]: r for r in csv.DictReader(f, delimiter="\t")}
    seen = set()
    counts = collections.Counter()
    for line in open(sample):
        cells = line.rstrip("\n").split("|")
        ident = cells[1].strip() if len(cells) > 2 else ""
        if ident in ("id", "---") or not ident:
            print(line, end="")
            continue
        j = judged[ident]
        assert j["class"] in CLASSES, ident
        assert ident not in seen, ident
        seen.add(ident)
        cells[-3] = f" {j['class']} "
        cells[-2] = " " + j["reason"].replace("|", "\\|") + " "
        print("|".join(cells))
        severity = cells[5].strip()
        counts[severity, j["class"]] += 1
        counts["all", j["class"]] += 1
    assert seen == set(judged), set(judged) - seen
    print()
    total = sum(counts["all", c] for c in CLASSES)
    for severity in ("error", "warning", "all"):
        row = ", ".join(f"{c} {counts[severity, c]}" for c in CLASSES)
        print(f"{severity}: {row}")
    good = counts["all", "useful"] + counts["all", "defensible"]
    print(f"useful or defensible: {good} of {total} ({good / total:.1%})")


if __name__ == "__main__":
    main()
