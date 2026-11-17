"""Compare the complexity findings of two runs of the evaluation's run.py
(tasks 1.5 and 3.2): which appeared, disappeared or changed.

Usage: compare.py <baseline out dir> <new out dir>
Prints one line per difference, then the counts, then the slowest runs
of the new run (from its runs.tsv).
"""
import csv
import glob
import json
import os
import sys


def findings(out):
    """{(repo, pr): {(path, line, rule): (severity, message)}}"""
    result = {}
    for report in glob.glob(os.path.join(out, "*", "*.json")):
        repo = os.path.basename(os.path.dirname(report))
        pr = os.path.splitext(os.path.basename(report))[0]
        with open(report) as f:
            data = json.load(f)
        result[repo, pr] = {
            (f["path"], f.get("line"), f["rule"]):
                (f["severity"], f["message"])
            for f in data["findings"]
            if f["rule"].startswith("complexity/")
        }
    return result


def main():
    base, new = (findings(d) for d in sys.argv[1:3])
    assert base.keys() == new.keys(), base.keys() ^ new.keys()
    counts = {"gone": 0, "new": 0, "changed": 0, "same": 0}
    for key in sorted(base):
        b, n = base[key], new[key]
        for f in sorted(b.keys() | n.keys(), key=str):
            where = f"{key[0]} {key[1]} {f[0]}:{f[1]}"
            if f not in n:
                counts["gone"] += 1
                print(f"gone     {where}  {b[f][0]} {b[f][1]}")
            elif f not in b:
                counts["new"] += 1
                print(f"new      {where}  {n[f][0]} {n[f][1]}")
            elif b[f] != n[f]:
                counts["changed"] += 1
                print(f"changed  {where}  {b[f][0]} {b[f][1]}")
                print(f"     ->  {where}  {n[f][0]} {n[f][1]}")
            else:
                counts["same"] += 1
    print(", ".join(f"{k} {v}" for k, v in counts.items()))
    with open(os.path.join(sys.argv[2], "runs.tsv")) as f:
        runs = sorted(csv.DictReader(f, delimiter="\t"), key=lambda r: -float(r["seconds"]))
    print("slowest:", ", ".join(f"{r['repo']} {r['pr']} {float(r['seconds']):.1f} s" for r in runs[:5]))


if __name__ == "__main__":
    main()
