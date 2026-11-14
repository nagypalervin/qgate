import json, sys, statistics
from collections import Counter
for name in sys.argv[2:]:
    rows = json.load(open(f"{sys.argv[1]}/{name}.json"))
    errors = [r["commit"][:8] for r in rows if "new_error" in r or "old_error" in r]
    rules = Counter(); same = 0
    for r in rows:
        if "new" not in r or "old" not in r: continue
        rest = [f for f in r["new"] if not f["rule"].startswith("tests/")]
        same += rest == r["old"]
        rules.update(f["rule"] for f in r["new"] if f["rule"].startswith("tests/"))
    nt = [r["new_time"] for r in rows]; ot = [r["old_time"] for r in rows]
    print(f"{name}: {len(rows)} commits, errors {errors}, other findings equal to 0.6.0 in {same}, tests findings {dict(rules)}, "
          f"time new mean {statistics.mean(nt):.2f} max {max(nt):.2f} / 0.6.0 mean {statistics.mean(ot):.2f} max {max(ot):.2f}, "
          f"range {rows[-1]['commit'][:8]}..{rows[0]['commit'][:8]}")
