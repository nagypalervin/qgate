"""Draw the 10 findings of Ervin's spot-check (design D3) from the
judged sample, with random.Random(20261117), in sample order.

Usage: spotcheck.py <judgements.tsv>
"""
import csv
import random
import sys

with open(sys.argv[1]) as f:
    ids = [r["id"] for r in csv.DictReader(f, delimiter="\t")]
print(" ".join(sorted(random.Random(20261117).sample(ids, 10), key=ids.index)))
