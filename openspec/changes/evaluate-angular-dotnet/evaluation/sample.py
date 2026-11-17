"""Draw the sample of design D3: every Error, and up to 30 Warnings per
repository, drawn with random.Random(20261117) from the Warnings in
report order (commits.tsv order, then each report's order).

Usage: sample.py <commits.tsv> <out dir>
Prints one Markdown row per sampled finding, with an id, ready to judge.
"""
import csv
import json
import os
import random
import sys

SEED = 20261117
WARNINGS = 30
PREFIX = {"CleanArchitecture": "CA", "server": "SV", "clients": "CL"}


def main():
    listing, out = sys.argv[1:3]
    with open(listing) as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    rng = random.Random(SEED)
    print("| id | repo | PR | kind | severity | rule | location | message | class | reason |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for repo in dict.fromkeys(r["repo"] for r in rows):
        errors, warnings = [], []
        for r in (r for r in rows if r["repo"] == repo):
            with open(os.path.join(out, repo, r["pr"] + ".json")) as f:
                for x in json.load(f)["findings"]:
                    item = (r, x)
                    if x["severity"] == "error":
                        errors.append(item)
                    elif x["severity"] == "warning":
                        warnings.append(item)
        picked = errors + rng.sample(warnings, min(WARNINGS, len(warnings)))
        for n, (r, x) in enumerate(picked, 1):
            where = f"{x.get('path', '')}:{x.get('line', '')}"
            note = " (hotspot)" if x.get("hotspot") else ""
            message = x["message"].replace("|", "\\|")
            print(f"| {PREFIX[repo]}{n} | {repo} | {r['pr']} | {r['kind']} | {x['severity']} | "
                  f"{x['rule']} | `{where}` | {message}{note} |  |  |")


if __name__ == "__main__":
    main()
