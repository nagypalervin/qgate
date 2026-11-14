import json, re, subprocess, sys
out, repos = sys.argv[1], sys.argv[2]
test = re.compile(r"(\.(spec|test)\.(ts|mts|cts|tsx)$)|(Tests?\.cs$)|(/[^/]*\.Tests?[^/]*/)|(Tests?/)")
for name in ["clients", "server", "CleanArchitecture", "eShop"]:
    rows = json.load(open(f"{out}/{name}.json"))
    for r in rows:
        for f in r["new"]:
            if f["rule"] != "tests/unchanged-test": continue
            unit = f["message"].split(" ")[0]
            cls = unit.split(".")[0]
            hits = []
            for p in r["files"]:
                if not test.search(p): continue
                show = subprocess.run(["git", "-C", f"{repos}/{name}", "show", f"{r['commit']}:{p}"], capture_output=True, text=True)
                if show.returncode == 0 and re.search(r"\b" + re.escape(cls) + r"\b", show.stdout):
                    hits.append(p)
            print(f"{name:17} {r['commit'][:8]} {f['path'].rsplit('/',1)[-1]:45} {cls:35} {'MENTIONED IN: ' + ', '.join(h.rsplit('/',1)[-1] for h in hits) if hits else '-'}")
