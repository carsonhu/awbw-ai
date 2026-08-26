"""Enforce per-file line budgets on the docs.

Documentation that grows without bound stops being read: an agent session loads
CLAUDE.md every time, and a bloated set of docs crowds out the code it is meant
to explain. Budgets are deliberately tight -- when one is hit, the fix is
usually to cut something stale rather than to raise the number.

Exit code 1 if any file is over budget.
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Loaded every session, so it gets the tightest budget of all.
#
# Budgets are raised only when a doc has grown because the *project* did, and
# never to avoid trimming something stale. architecture.md went from 90 to 110
# when the workspace went from two crates to five; that is real content, and
# squeezing prose to save single lines was costing more than the limit saved.
#
# decisions.md is append-only by design, so it grows with the project by
# construction. Its discipline is per-entry -- a few lines each, and delete an
# entry when it stops being true -- rather than a fixed total.
#
# architecture.md and workflow.md went up again when training was added: a
# network, a cloning loop and a way to rate a checkpoint are a stage the project
# did not have, not padding around one it did. decisions.md went up with the RL
# stage for the same reason -- three of its hardest-won entries are about PPO,
# and they were paid for in silently wrong runs.
BUDGETS = {
    "CLAUDE.md": 45,
    "README.md": 60,
    "docs/architecture.md": 115,
    "docs/rules.md": 90,
    "docs/verification.md": 90,
    "docs/decisions.md": None,  # per entry; see ENTRY_BUDGET
    "docs/workflow.md": 85,
}

# decisions.md is measured per entry instead of in total. It is append-only by
# design, so a total is the wrong instrument -- it was raised three times in one
# day against content that was neither stale nor padded, which is a budget
# reporting on the calendar rather than on the writing. What actually keeps it
# readable is that no single entry sprawls.
ENTRY_BUDGET = 6


def entry_failures(path, budget):
    """Over-long entries in an append-only doc. Entries are paragraphs."""
    out = []
    for block in path.read_text(encoding="utf-8").split("\n\n"):
        lines = [ln for ln in block.splitlines() if ln.strip()]
        if not lines or not lines[0].startswith("**"):
            continue  # headings and the preamble
        if len(lines) > budget:
            title = lines[0].split("**")[1] if "**" in lines[0] else lines[0]
            out.append(f'{len(lines)} lines: "{title[:52]}"')
    return out


# Everything under docs/ needs a budget, so a new file cannot slip in unbounded.
def main():
    failures = []
    rows = []

    for name, budget in sorted(BUDGETS.items()):
        path = ROOT / name
        if not path.exists():
            failures.append(f"{name}: missing")
            continue
        lines = len(path.read_text(encoding="utf-8").splitlines())
        if budget is None:
            over = entry_failures(path, ENTRY_BUDGET)
            rows.append((name, lines, f"<={ENTRY_BUDGET}/entry"))
            failures.extend(f"{name}: entry runs {o}" for o in over)
            continue
        rows.append((name, lines, budget))
        if lines > budget:
            failures.append(f"{name}: {lines} lines, budget {budget}")

    for doc in sorted((ROOT / "docs").glob("*.md")):
        rel = f"docs/{doc.name}"
        if rel not in BUDGETS:
            failures.append(f"{rel}: no budget set; add one to tools/check_docs.py")

    width = max((len(r[0]) for r in rows), default=0)
    total = sum(r[1] for r in rows)
    for name, lines, budget in rows:
        if isinstance(budget, str):
            bar = "per entry"
        else:
            bar = "over" if lines > budget else f"{budget - lines} spare"
        print(f"  {name:<{width}}  {lines:>4} / {str(budget):<11} {bar}")
    print(f"  {'total':<{width}}  {total:>4} lines across {len(rows)} files")

    if failures:
        print("\nover budget:")
        for f in failures:
            print(f"  {f}")
        print("\nTrim stale content before raising a budget.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
