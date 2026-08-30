"""The docs system: budgets, an index that cannot go stale, and an archive.

Three kinds of document live under `docs/`, and they need opposite rules.

*Reference* docs answer "how does this work now". There is a fixed, small set of
them and they are *edited*, never appended to, so a line budget is the right
instrument: hitting one means something in the file is stale.

The *log* is where volume actually comes from. Every experiment worth trusting
later produces numbers, and numbers rot the moment they share a file with prose
that has to stay current. So log entries are dated, immutable, and unbudgeted in
total -- writing a new one costs nothing to anybody reading the reference set.

The *archive* is for documents the project has outgrown. Deleting them loses the
reasoning; leaving them in place makes the reader guess which files are true.

Lookup is one generated index, `docs/README.md`. Each doc carries its own
one-line hook as a blockquote under its title; the index is those hooks
assembled, and `check` fails if it has drifted. An index nobody has to remember
to update is the only kind that stays complete.

    python tools/docs.py            # check budgets, conventions, index
    python tools/docs.py index      # rewrite docs/README.md
    python tools/docs.py find fog   # which doc talks about this

Exit code 1 if anything is over budget or out of convention.
"""

import re
import sys
from pathlib import Path

# The docs are UTF-8 and full of em-dashes; a Windows console is often neither.
# Mangling one character of a hook beats refusing to print the index.
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(errors="replace")

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
LOG = DOCS / "log"
ARCHIVE = DOCS / "archive"
INDEX = DOCS / "README.md"

# Loaded every session, so CLAUDE.md gets the tightest budget of all.
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
# architecture.md and workflow.md are two over their old limits, and only
# because every doc now owes the index a hook line. The rest paid for theirs by
# turning the prose they already opened with into the blockquote.
BUDGETS = {
    "CLAUDE.md": 45,
    "README.md": 60,
    "docs/architecture.md": 117,
    "docs/rules.md": 90,
    "docs/verification.md": 90,
    "docs/decisions.md": None,  # per entry; see ENTRY_BUDGET
    # Up 14 for writing replays out, which is a thing the project can now do
    # rather than a longer way of saying what it already did. Up 7 more for the
    # knobs that fit PPO to this game rather than to Atari -- the credit
    # horizon, the discount's unit, and what the shaping measures.
    #
    # Up 8 more for reading a policy against the human corpus rather than
    # against a bot: `play_diag.py`, and the two flags that answer what it
    # found. The trim that paid for part of it was real staleness -- `--amp`
    # had been advice about a flag `ppo.py` does not have, and the documented
    # recipe said `--gamma 0.99` where all eight logged runs passed `--lam`.
    "docs/workflow.md": 116,
    # The network reviewed against the systems it descends from (AlphaZero
    # trunk, AlphaStar decoder), and the ordered list of what to change.
    # Reference, not a log entry: it states what the design is and why, and
    # is edited as the design moves.
    "docs/network.md": 90,
    # The agenda of record. Added after a scope decision (the Tier-4 CO
    # boundary) survived only in a dead conversation and got restated wrong:
    # plans are reference material -- edited as they change, budgeted so a
    # completed item becomes a pointer to its log entry instead of residue.
    "docs/plan.md": 70,
}

# decisions.md is measured per entry instead of in total. It is append-only by
# design, so a total is the wrong instrument -- it was raised three times in one
# day against content that was neither stale nor padded, which is a budget
# reporting on the calendar rather than on the writing. What actually keeps it
# readable is that no single entry sprawls.
ENTRY_BUDGET = 6

# A log entry records one experiment: the question, the commands, the numbers,
# and what they mean. That fits comfortably here. An entry pushing this limit is
# usually two experiments, or is arguing a conclusion that belongs in
# decisions.md with the entry as its evidence.
LOG_BUDGET = 60

LOG_NAME = re.compile(r"^(\d{4}-\d{2}-\d{2})-[a-z0-9][a-z0-9-]*\.md$")

# The index is generated, so its prose lives here -- edit the generator, same as
# for data.rs. Everything below the preamble comes from the docs themselves.
PREAMBLE = """# Docs

<!-- Generated by tools/docs.py. Edit the docs, or the generator, not this. -->

Three kinds of document, with different rules:

- **Reference** answers *how does this work now*. Kept current, edited in place,
  budgeted by line count. A budget hit means something here is stale.
- **The log** records *what happened when we measured it*. Dated, immutable, and
  written once -- a superseded result is answered by a newer entry, never by
  editing the old one.
- **The archive** holds documents the project has outgrown. Kept for their
  reasoning. **Nothing in it is authoritative.**

`decisions.md` is its own thing: append-only, a few lines per entry, and the
first place to look before re-opening a settled question. An entry states the
conclusion; the log entry it links to holds the numbers behind it.

## Adding to this

After an experiment worth trusting later, write `log/YYYY-MM-DD-slug.md`: the
question, the commands, the numbers, and what they mean. If it settles
something, add a few lines to `decisions.md` linking it. Retire a reference doc
by moving it to `archive/` under a `**Retired**` line saying when, why and what
replaced it -- deleting it loses the reasoning, leaving it in place misleads.

Every doc opens with `# Title` and a `> one-line hook`; the hook is what appears
in the tables below, so write it for someone deciding whether to open the file.
Then `python tools/docs.py index`.
"""


def parse(path):
    """(title, hook) -- the H1, then the blockquote saying when to read it."""
    title, hook = None, []
    for line in path.read_text(encoding="utf-8").splitlines():
        if title is None:
            if line.startswith("# "):
                title = line[2:].strip()
            continue
        if line.startswith(">"):
            hook.append(line.lstrip(">").strip())
        elif hook or line.strip():
            break
    return title, " ".join(hook)


def rel(path):
    return path.relative_to(ROOT).as_posix()


def reference_docs():
    return sorted(p for p in DOCS.glob("*.md") if p != INDEX)


def log_entries():
    """Newest first -- the filename's date sorts, which is why it leads."""
    if not LOG.is_dir():
        return []
    return sorted(LOG.glob("*.md"), reverse=True)


def archived_docs():
    return sorted(ARCHIVE.glob("*.md")) if ARCHIVE.is_dir() else []


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


def table(rows, headers):
    out = ["| " + " | ".join(headers) + " |", "|" + "---|" * len(headers)]
    out += ["| " + " | ".join(r) + " |" for r in rows]
    return out


def render():
    """The index, assembled from every doc's own hook."""
    out = PREAMBLE.splitlines()

    out += ["", "## Reference", ""]
    rows = []
    for path in reference_docs():
        title, hook = parse(path)
        rows.append([f"[{path.name}]({path.name})", hook or title or ""])
    out += table(rows, ["doc", "read when"])

    entries = log_entries()
    out += ["", f"## Log ({len(entries)})", ""]
    if entries:
        rows = []
        for path in entries:
            title, hook = parse(path)
            date = path.name[:10]
            link = f"[{title or path.stem}](log/{path.name})"
            rows.append([date, link, hook])
        out += table(rows, ["date", "entry", "what it found"])
    else:
        out += ["Nothing recorded yet."]

    archived = archived_docs()
    if archived:
        out += ["", f"## Archive ({len(archived)})", "", "Not authoritative.", ""]
        rows = []
        for path in archived:
            title, hook = parse(path)
            rows.append([f"[{path.name}](archive/{path.name})", hook or title or ""])
        out += table(rows, ["doc", "was"])

    return "\n".join(out) + "\n"


def conventions():
    """Every doc needs a title and a hook; the index is built out of them."""
    failures = []
    for path in reference_docs() + log_entries() + archived_docs():
        title, hook = parse(path)
        if not title:
            failures.append(f"{rel(path)}: no `# Title` line")
        if not hook:
            failures.append(
                f"{rel(path)}: no `> ...` hook under the title; "
                "one line on when to read it, and it becomes the index entry"
            )

    for path in log_entries():
        if not LOG_NAME.match(path.name):
            failures.append(f"{rel(path)}: log entries are named YYYY-MM-DD-slug.md")
        lines = len(path.read_text(encoding="utf-8").splitlines())
        if lines > LOG_BUDGET:
            failures.append(f"{rel(path)}: {lines} lines, budget {LOG_BUDGET}")

    for path in archived_docs():
        if "Retired" not in path.read_text(encoding="utf-8"):
            failures.append(
                f"{rel(path)}: no `**Retired ...**` line saying when and why, "
                "and what to read instead"
            )
    return failures


def check():
    failures, rows = [], []

    for name, budget in sorted(BUDGETS.items()):
        path = ROOT / name
        if not path.exists():
            failures.append(f"{name}: missing")
            continue
        lines = len(path.read_text(encoding="utf-8").splitlines())
        if budget is None:
            failures.extend(
                f"{name}: entry runs {o}" for o in entry_failures(path, ENTRY_BUDGET)
            )
            rows.append((name, lines, f"<={ENTRY_BUDGET}/entry"))
            continue
        rows.append((name, lines, budget))
        if lines > budget:
            failures.append(f"{name}: {lines} lines, budget {budget}")

    # A new reference doc cannot slip in unbounded. The log and the archive are
    # deliberately open -- that is what they are for.
    for path in reference_docs():
        if rel(path) not in BUDGETS:
            failures.append(
                f"{rel(path)}: no budget set. Add one to tools/docs.py, or file it "
                "under docs/log/ if it is a dated result rather than reference."
            )

    failures += conventions()

    stale = not INDEX.exists() or INDEX.read_text(encoding="utf-8") != render()
    if stale and not failures:
        failures.append(f"{rel(INDEX)}: out of date; run `python tools/docs.py index`")

    width = max((len(r[0]) for r in rows), default=0)
    for name, lines, budget in rows:
        bar = "per entry" if isinstance(budget, str) else (
            "over" if lines > budget else f"{budget - lines} spare"
        )
        print(f"  {name:<{width}}  {lines:>4} / {str(budget):<11} {bar}")
    log_lines = sum(len(p.read_text(encoding="utf-8").splitlines()) for p in log_entries())
    print(f"  {'reference':<{width}}  {sum(r[1] for r in rows):>4} lines")
    print(f"  {'log':<{width}}  {log_lines:>4} lines across {len(log_entries())} entries")
    print(f"  {'archive':<{width}}  {len(archived_docs()):>4} docs")

    if failures:
        print("\nproblems:")
        for f in failures:
            print(f"  {f}")
        print("\nTrim stale content before raising a budget.")
        return 1
    return 0


def write_index():
    # Explicit newline, so the index does not differ from itself across
    # platforms and fail its own staleness check.
    with open(INDEX, "w", encoding="utf-8", newline="\n") as out:
        out.write(render())
    print(f"wrote {rel(INDEX)}")
    return 0


def find(terms):
    """Rank docs by a term's presence in the name, title, hook, then the body."""
    hits = []
    for path in reference_docs() + log_entries() + archived_docs():
        title, hook = parse(path)
        body = path.read_text(encoding="utf-8").lower()
        score = 0
        for term in (t.lower() for t in terms):
            score += 4 * (term in path.name.lower())
            score += 4 * (term in (title or "").lower())
            score += 2 * (term in hook.lower())
            score += min(body.count(term), 3)
        if score:
            hits.append((score, path, hook))

    if not hits:
        print(f"no doc mentions {' '.join(terms)}")
        return 1
    # A common word appears in every doc, so an unfiltered list is the same
    # answer as no search. Anything well behind the best match is noise.
    hits.sort(key=lambda h: -h[0])
    floor = hits[0][0] / 3
    for score, path, hook in hits:
        if score >= floor:
            print(f"  {rel(path):<34} {hook[:76]}")
    return 0


def main(argv):
    command = argv[0] if argv else "check"
    if command == "check":
        return check()
    if command == "index":
        return write_index()
    if command == "find":
        if len(argv) < 2:
            print("usage: docs.py find <term> [term ...]")
            return 2
        return find(argv[1:])
    print(__doc__.strip().splitlines()[-1])
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
