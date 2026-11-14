import json, sys, re
name = sys.argv[2]
rows = json.load(open(f"{sys.argv[1]}/{name}.json"))
test = re.compile(r"(\.(spec|test)\.(ts|mts|cts|tsx)$)|(Tests?/)|(Tests?\.cs$)|(\.Tests?\.)")
for r in rows:
    fs = [f for f in r["new"] if f["rule"].startswith("tests/")]
    if not fs: continue
    tests = [p for p in r["files"] if test.search(p)]
    print(f"== {r['commit'][:8]} {r['subject'][:80]}  ({len(r['files'])} files; tests touched: {tests[:6]}{' ...' if len(tests)>6 else ''})")
    for f in fs:
        print(f"   {f['rule'][6:]:15} {f['path']}:{f.get('line')}  {f['message'][:150]}")
