"""Write findings.md: every tests finding of the first run (out/), with
its category, and whether the second run (out2/) still reports it.
Usage: classify.py <out dir> <out2 dir> > findings.md
The categories are the hand sort recorded in design.md, by file name."""
import json, sys

SHOULD, NONE, HELPER, MISS = "should", "needs no test", "test code", "miss"
CATEGORY = {
    "clients": {
        **{f: SHOULD for f in [
            "vault.component.ts", "vault-items-table.component.ts", "access-rules.component.ts",
            "native-messaging.main.ts", "free-trial-text.resolver.ts", "sm-access-token.utils.ts",
            "sm-grantee.response.ts", "sm-secret-project-names.ts", "sm-secret-request.ts",
            "trash-selection.ts", "lease-messages.ts", "popup-revealed-cipher.service.ts"]},
        **{f: HELPER for f in [
            "sm-access-test-utils.ts", "sm-test-utils.ts", "leased-cipher.state.spec-util.ts"]},
    },
    "server": {"OrganizationsController.cs": SHOULD, "SendInviteRemindersJob.cs": SHOULD,
               "AccountController.cs": MISS},
    "CleanArchitecture": {f: NONE for f in [
        "UpdateTodoItem.cs", "UpdateTodoItemDetail.cs", "IEndpointGroup.cs",
        "ProblemDetailsExceptionHandler.cs", "IdentityApiOperationTransformer.cs",
        "BearerSecuritySchemeTransformer.cs"]},
    "eShop": {"CatalogService.cs": SHOULD, "cart-helpers.ts": HELPER},
}
DEFAULT = {"clients": NONE, "server": NONE, "CleanArchitecture": SHOULD, "eShop": NONE}

out, out2 = sys.argv[1:3]
print("# Evaluation findings (task 2.6)\n")
print("Every `tests/` finding of the first run, sorted by hand (see design.md,")
print("\"Evaluation\"). \"After\" says whether the run with the three decisions")
print("still reports it.\n")
totals = {}
for name in ["clients", "server", "CleanArchitecture", "eShop"]:
    later = {(r["commit"], f["path"]) for r in json.load(open(f"{out2}/{name}.json"))
             for f in r["new"] if f["rule"].startswith("tests/")}
    print(f"## {name}\n")
    print("| commit | rule | file | category | after |")
    print("|---|---|---|---|---|")
    for r in json.load(open(f"{out}/{name}.json")):
        for f in r["new"]:
            if not f["rule"].startswith("tests/"):
                continue
            file = f["path"].rsplit("/", 1)[-1]
            category = CATEGORY[name].get(file, DEFAULT[name])
            totals[(name, category)] = totals.get((name, category), 0) + 1
            after = "yes" if (r["commit"], f["path"]) in later else "no"
            print(f"| {r['commit'][:8]} | {f['rule'][6:]} | `{f['path']}:{f['line']}` | {category} | {after} |")
    print()
print("## Totals\n")
print("| repository | should | needs no test | test code | miss |")
print("|---|---|---|---|---|")
for name in ["clients", "server", "CleanArchitecture", "eShop"]:
    print(f"| {name} | " + " | ".join(str(totals.get((name, c), 0)) for c in [SHOULD, NONE, HELPER, MISS]) + " |")
