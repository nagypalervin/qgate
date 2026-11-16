"""Write findings.md: every finding with hotspot context in the run with
tests left out of the ranking, with its category and whether the first
run (tests ranked) had it too. Usage: classify_hs.py <out> <out2> > findings.md
The categories are the hand sort recorded in design.md."""
import json, re, sys

SUPPORT = re.compile(r"Seeder|story\.helper")
ROOTS = re.compile(r"main\.background\.ts$|service-container\.ts$")
out, out2 = sys.argv[1:3]
print("# Hotspot evaluation findings (task 2.5)\n")
print("Every finding with hotspot context in the second run (tests not ranked),")
print("sorted by hand (see design.md, \"Evaluation\"). CleanArchitecture and")
print("eShop have none: no file changed in 5 commits of a window.\n")
totals = {}
for name in ["server", "clients"]:
    first = {(r["commit"], f["path"], f.get("line"), f["rule"])
             for r in json.load(open(f"{out}/{name}.json")) for f in r["new"] if "hotspot" in f}
    print(f"## {name}\n")
    print("| commit | rule | file | severity | commits | complexity | category |")
    print("|---|---|---|---|---|---|---|")
    for r in json.load(open(f"{out2}/{name}.json")):
        for f in r["new"]:
            if "hotspot" not in f:
                continue
            h = f["hotspot"]
            if SUPPORT.search(f["path"]):
                cat = "support code"
            elif ROOTS.search(f["path"]):
                cat = "composition root"
            elif h["complexity"] >= 50:
                cat = "central code"
            else:
                cat = "small file"
            assert (r["commit"], f["path"], f.get("line"), f["rule"]) in first
            totals[(name, cat)] = totals.get((name, cat), 0) + 1
            print(f"| {r['commit'][:8]} | {f['rule']} | `{f['path']}:{f.get('line')}` | {f['severity']} | {h['commits']} | {h['complexity']} | {cat} |")
    print()
print("## Totals\n")
cats = ["central code", "composition root", "small file", "support code"]
print("| repository | " + " | ".join(cats) + " |")
print("|---|---|---|---|---|")
for name in ["server", "clients"]:
    print(f"| {name} | " + " | ".join(str(totals.get((name, c), 0)) for c in cats) + " |")
