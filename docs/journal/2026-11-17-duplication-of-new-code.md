# 2026-11-17: duplication-of-new-code

**Went well:**
- **A spike set the bar before any code.** A Python script over `git
  diff -U0` replayed the rule on the 150 evaluation pull requests first,
  so the proposal could name which judged findings must stay and which
  must go. The Rust rule was then measured against that list, not
  against a feeling.
- **The replay met the bar with nothing new.** Duplication findings fell
  from 279 to 139. Useful 6 of 6 and defensible 12 of 13 stay (SV30
  goes, as expected), 20 of 22 noise go, every finding of another rule
  is identical, and no finding appeared that was not there before. The
  slowest pull request runs within 3 % of before.
- **Mutants kept the pairing honest.** Each piece of the rule (own file
  first, consumption, runs of three, empty runs, trimming, the
  written-line condition, moved holders, the slide) was broken by hand,
  and each break failed a test. One trap was caught on the way: a file
  restored with its old mtime let cargo keep the mutant's build, so the
  first results were rerun with fresh mtimes.
- **qgate checked itself twice.** On PR 1 it flagged a new Myers
  function of complexity 18 and a test oracle copied from complexity;
  on PR 2 a new pairing function of complexity 11. All three were split
  or shared; the +1 left on each of three functions is one condition of
  the rule each.
- **The review found an untested order.** Every fixture with a moved
  copy happened to have it first by path, so sorting moved holders first
  was untested; a mirrored fixture now runs both orders.

**Had to step in:** These were Ervin's calls:
- in the exploration: only code the change wrote counts; lines are
  matched by trimmed text in runs of three; pairing is one to one, with
  consumption; the threshold stays at 50 tokens; two pull requests, with
  the spike's replay as the bar;
- during the apply: a file the block only moved to is no new copy (an
  existing test found that a moved copy, first by path, hid a written
  one);
- during the re-measurement: the line diff slides changes down as git
  does, which brought SV20 back; and SV23 staying instead of SV8 is
  accepted;
- merging.

**Surprise:**
- **The spike was right about lines and wrong about tokens.** SV8 and
  SV23 have exactly the spike's written lines, but the spike counted
  tokens with a regular expression. qgate's tokenizer counts 40 for SV8
  and 55 for SV23, so the threshold of 50 swaps them.
- **Two correct diffs can disagree.** Our Myers and git's both find a
  shortest edit, but for a test appended after a similar test git
  slides the insertion down and ours did not: the copy's tail looked
  kept, and a defensible finding (SV20) went silent. Tie-breaking is
  behavior.

**Effort:** a long day; 2 PRs, the replay run four times plus debug
runs, two review rounds, one per PR.

**Blog idea:** "Measure the rule before you write it." A throwaway script
on real history turned "less noise" into a list of named findings, and
that list caught two things the fixtures could not: a diff's
tie-breaking, and a token estimate that was off by just enough.
