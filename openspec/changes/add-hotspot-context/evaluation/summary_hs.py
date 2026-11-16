"""Summarize the hotspot evaluation. Usage: summary_hs.py <out dir> <repo>..."""
import json, re, statistics, sys

NOTE = re.compile(r"; hotspot: \d+ commits in 6 months, complexity \d+$")
out = sys.argv[1]
for name in sys.argv[2:]:
    rows = json.load(open(f"{out}/{name}.json"))
    errors = [r["commit"][:8] for r in rows if "new_error" in r or "old_error" in r]
    stderr = sorted({r["new_stderr"].strip() for r in rows if r["new_stderr"].strip()})
    same = annotated = warnings = 0
    files = set()
    listed = []
    for r in rows:
        if "new" not in r or "old" not in r:
            continue
        stripped = []
        for f in r["new"]:
            g = {k: v for k, v in f.items() if k != "hotspot"}
            g["message"] = NOTE.sub("", g["message"])
            stripped.append(g)
            if "hotspot" in f:
                annotated += 1
                warnings += f["severity"] == "warning"
                files.add(f["path"])
                listed.append((r["commit"][:8], f["rule"], f["path"], f.get("line"), f["severity"],
                               f["hotspot"]["commits"], f["hotspot"]["complexity"]))
        same += stripped == r["old"]
    nt = [r["new_time"] for r in rows]; ot = [r["old_time"] for r in rows]; lt = [r["log_time"] for r in rows]
    print(f"{name}: {len(rows)} commits, errors {errors}, stderr {stderr}, equal to 0.7.0 without the note: {same}, "
          f"annotated {annotated} ({warnings} warnings) in {len(files)} files; "
          f"time new {statistics.mean(nt):.2f} (max {max(nt):.2f}) / 0.7.0 {statistics.mean(ot):.2f} (max {max(ot):.2f}); "
          f"git log {statistics.mean(lt):.2f} (max {max(lt):.2f})")
    for row in listed:
        print("   ", *row)
