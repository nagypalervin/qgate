"""Compare a second run with the first, finding by finding (design.md,
"Second measurement: 0.10.0"; and in the pilot, after a fix).

Usage: remeasure.py <findings.md> <first out dir> <second out dir>

- Each judged finding of findings.md: kept (same rule, path and line in
  the second run), moved (same rule and path, overlapping lines) or gone.
- The criteria over the judged findings that are kept or moved: useful or
  defensible of all, and Errors classed noise.
- Every finding of the second run that no finding of the first run of the
  same rule and path overlaps: these are new and need a judgement.
"""
import collections
import glob
import json
import os
import re
import sys


def load(out):
    reports = {}
    for path in glob.glob(os.path.join(out, "*", "*.json")):
        repo = os.path.basename(os.path.dirname(path))
        pr = os.path.splitext(os.path.basename(path))[0]
        with open(path) as f:
            reports[repo, pr] = json.load(f)["findings"]
    return reports


def overlaps(f, g):
    return (
        f["rule"] == g["rule"]
        and f["path"] == g["path"]
        and f["line"] <= g.get("end_line", g["line"])
        and g["line"] <= f.get("end_line", f["line"])
    )


def main():
    table, first_dir, second_dir = sys.argv[1:4]
    first, second = load(first_dir), load(second_dir)
    assert first.keys() == second.keys(), first.keys() ^ second.keys()

    judged = []
    for line in open(table):
        c = [x.strip() for x in line.split("|")]
        # Ids as sample.py writes them: SV12, or partner-web-12.
        if len(c) > 10 and re.match(r"^[A-Za-z][\w.-]*\d+$", c[1]):
            path, ln = c[7].strip("`").rsplit(":", 1)
            judged.append((c[1], c[2], c[3], c[5], c[6], path, int(ln), c[9]))

    states = collections.Counter()
    kept_classes = collections.Counter()
    for ident, repo, pr, severity, rule, path, ln, cls in judged:
        now = second[repo, pr]
        probe = {"rule": rule, "path": path, "line": ln}
        if any(g["rule"] == rule and g["path"] == path and g["line"] == ln for g in now):
            state = "kept"
        elif any(overlaps(probe, g) for g in now):
            state = "moved"
        else:
            state = "gone"
        states[cls, state] += 1
        if state != "gone":
            kept_classes[severity, cls] += 1
        else:
            print(f"gone   {ident:5} {cls:10} {severity:7} {rule} {repo} {pr} {path}:{ln}")
        if state == "moved":
            print(f"moved  {ident:5} {cls:10} {severity:7} {rule} {repo} {pr} {path}:{ln}")

    print("class, state -> count")
    for key, count in sorted(states.items()):
        print("  ", key, count)
    total = sum(kept_classes.values())
    if not total:
        sys.exit("no judged finding is still reported")
    good = sum(v for (_, cls), v in kept_classes.items() if cls != "noise")
    false_errors = kept_classes["error", "noise"]
    print(f"still reported: {total}; useful or defensible: {good} ({100 * good / total:.1f}%)")
    print(f"Errors still reported: {sum(v for (s, _), v in kept_classes.items() if s == 'error')},"
          f" classed noise: {false_errors}")

    new = []
    for key in sorted(second):
        for g in second[key]:
            if not any(overlaps(f, g) for f in first[key]):
                new.append((key, g))
    print("new findings:", len(new))
    for (repo, pr), g in new:
        print(f"   {repo} {pr} {g['severity']} {g['rule']} {g['path']}:{g['line']} {g['message']}")


if __name__ == "__main__":
    main()
