#!/usr/bin/env python3
"""Write, check and post an issue brief for the issue-working agent; manage the queue's labels and
board from the orchestrator's side.

Usage (from the repository root):
    brief_issue.py scaffold --title T --slug S [--task ID] [--base-commit SHA] [--scope GLOB]...
                                    Print a draft: the standard sections filled from the
                                    configuration, [[FILL]] placeholders, and the brief marker.
    brief_issue.py labels [--task ID]
                                    The repository's labels by family and its open milestones,
                                    with the plan label and milestones the task ID suggests.
    brief_issue.py check DRAFT --title T [--label L]... [--milestone M] [--allow-duplicate]
                                    Every check `post` makes, posting nothing. For a draft written
                                    by hand, --slug S [--task ID] [--base-commit SHA] [--scope G]
                                    add the brief marker (check and post).
    brief_issue.py post DRAFT --title T [--label L]... [--milestone M] [--dry-run]
                                    Check, then open the issue with the task label and put it on
                                    the board as queued. --dry-run prints it and posts nothing.
    brief_issue.py answer N --body-file F [--requeue] [--dry-run]
                                    Comment an answer or amendment on a brief, begun with the
                                    orchestrator's tag (the posted brief's body carries none, as
                                    the task label and its marker identify it); --requeue also
                                    removes its state labels (claimed, done, blocked) so that a
                                    worker takes it again, continuing from the pushed lane.
    brief_issue.py setup-labels [--dry-run]
                                    Create the four agent labels where they are missing.
    brief_issue.py board-setup [--dry-run]
    brief_issue.py board-sync [--dry-run]
                                    Find or create the board and check its status options; move
                                    every open brief's card to the status its labels imply. Both
                                    skip cleanly without the token's `project` scope.

Exit codes: 0 done; 1 a check failed; 2 usage; 3 the issue failed the trust check.
Standard library only; Python 3.9+, run as `python3 -I`. Tests: test_brief_issue.py beside this.
"""

from __future__ import annotations

import argparse
import re
import shlex
import sys
from pathlib import Path
from string import Template
from typing import Any, Dict, List, Optional, Sequence

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "gh-collab"))

import ghcollab as gc  # noqa: E402  (the shared module's directory is put on the path first)

TEMPLATE = Path(__file__).resolve().parent.parent / "templates" / "brief.md"
DEFAULT_REPORT = ["the branch and its commit hashes", "the gate's exit line",
                  "deviations, and deferred findings, one line each", "any open question, with your lean"]
TOP_LEVEL_PATH_RE = re.compile(r"`([A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.@-]+)+/?)`")


def plan_of(task: str, cfg: Dict[str, Any]) -> str:
    if not task or not cfg.get("task_id_pattern"):
        return ""
    m = re.fullmatch(cfg["task_id_pattern"], task)
    if not m:
        raise gc.CollabError(f"{task!r} does not match task_id_pattern", gc.EXIT_USAGE)
    return m.groupdict().get("plan") or ""


def base_commit(ctx: gc.Ctx, given: str, fetch: bool) -> str:
    cfg, git = ctx.cfg, ctx.git
    remote_base = f"{cfg['remote']}/{cfg['base_branch']}"
    if fetch:
        git.run("fetch", "--quiet", cfg["remote"], cfg["base_branch"])
    sha = given or git("rev-parse", "--short=8", remote_base)
    if not git.has_ref(sha):
        raise gc.CollabError(f"base commit {sha} is not in this clone")
    if not git.is_ancestor(sha, remote_base):
        raise gc.CollabError(f"base commit {sha} is not on {remote_base}: push it before the brief names it")
    return sha


def scaffold(ctx: gc.Ctx, title: str, slug: str, task: str, given_base: str, scope: List[str], fetch: bool) -> str:
    cfg = ctx.cfg
    if not gc.SLUG_RE.fullmatch(slug):
        raise gc.CollabError(f"slug {slug!r} must be lower-case letters, digits, '.', '_' or '-'", gc.EXIT_USAGE)
    plan_of(task, cfg)
    sha = base_commit(ctx, given_base, fetch)
    lane = f"{cfg['branch_prefix']}<this issue's number>-{slug}"
    commit = cfg["commit"]
    subject = f"<type>(<scope>): {task} <summary>" if task else "<type>(<scope>): <summary>"
    rules = [f"Use conventional commits: `{subject}`."
             + (f" End every commit message with exactly this trailer line:\n  `{commit['trailer']}`"
                if commit.get("trailer") else "")]
    rules += [
        "Never add Claude session links anywhere: not in commits, comments or files.",
        f"Push only `{lane}`. Never push `{cfg['base_branch']}` or any other branch, never force-push, "
        "and never open or merge a pull request.",
        "This repository is public: put no personal details, email addresses, local paths or secrets in comments.",
    ]
    rules += list(cfg["brief"].get("rules", []))
    report = list(cfg["brief"].get("report") or DEFAULT_REPORT)
    setup = cfg["brief"].get("setup", [])
    meta = {"slug": slug, "base": cfg["base_branch"], "base_commit": sha}
    if task:
        meta["task"] = task
    if scope:
        meta["scope"] = scope
    values = {
        "title": title,
        "remote": cfg["remote"],
        "base": cfg["base_branch"],
        "base_commit": sha,
        "lane": lane,
        "setup": ("- Setup:\n" + "\n".join(f"  - {s}" for s in setup) + "\n") if setup else "",
        "gate_command": gc.fill(cfg["worker"].get("gate_command") or "<the repository's gate>", cfg),
        "rules": "".join(f"- {r}\n" for r in rules),
        "report": "\n".join(f"- {r};" for r in report),
        "claimed": cfg["labels"]["claimed"],
        "done": cfg["labels"]["done"],
        "blocked": cfg["labels"]["blocked"],
        "marker": gc.make_marker("brief", meta),
    }
    return Template(TEMPLATE.read_text(encoding="utf-8")).safe_substitute(values)


def with_marker(ctx: gc.Ctx, body: str, slug: str, task: str, given_base: str, scope: List[str],
                fetch: bool) -> str:
    """A hand-written draft gets its brief marker from the flags; a scaffolded one keeps its own."""
    has = any(kind == "brief" for kind, _ in gc.find_markers(body))
    if not slug:
        return body
    if has:
        raise gc.CollabError("the draft already holds a brief marker: drop --slug and the other marker flags",
                             gc.EXIT_USAGE)
    if not gc.SLUG_RE.fullmatch(slug):
        raise gc.CollabError(f"slug {slug!r} must be lower-case letters, digits, '.', '_' or '-'", gc.EXIT_USAGE)
    meta: Dict[str, Any] = {"slug": slug, "base": ctx.cfg["base_branch"],
                            "base_commit": base_commit(ctx, given_base, fetch)}
    if task:
        meta["task"] = task
    if scope:
        meta["scope"] = scope
    return body.rstrip() + "\n\n" + gc.make_marker("brief", meta) + "\n"


def draft_meta(body: str) -> Dict[str, Any]:
    metas = [data for kind, data in gc.find_markers(body) if kind == "brief"]
    if len(metas) != 1:
        raise gc.CollabError(f"the draft holds {len(metas)} brief markers, not one: start from `scaffold`")
    return metas[0]


def repo_labels(gh: gc.Gh) -> List[str]:
    return [lab["name"] for lab in gh.json("label", "list", "--repo", gh.repo, "--limit", "500", "--json", "name") or []]


def open_milestones(gh: gc.Gh) -> List[Dict[str, Any]]:
    return gh.json("api", f"repos/{gh.repo}/milestones?state=open&per_page=100") or []


def suggest(cfg: Dict[str, Any], task: str, labels: List[str], milestones: List[Dict[str, Any]]) -> Dict[str, Any]:
    plan = plan_of(task, cfg)
    label = cfg.get("plan_label", "").format(plan=plan) if plan else ""
    found = [m["title"] for m in milestones
             if plan and re.search(r"(?<![\w])" + re.escape(plan) + r"(?![\w])",
                                   f"{m.get('title', '')} {m.get('description') or ''}")]
    return {"plan_label": label if label in labels else "", "milestones": found}


def check(ctx: gc.Ctx, body: str, title: str, labels: List[str], milestone: str, allow_duplicate: bool,
          fetch: bool) -> List[gc.Finding]:
    cfg, gh, git = ctx.cfg, ctx.gh, ctx.git
    found: List[gc.Finding] = []

    def err(rule: str, excerpt: str = "") -> None:
        found.append(gc.Finding("error", rule, 0, excerpt))

    def warn(rule: str, excerpt: str = "") -> None:
        found.append(gc.Finding("warning", rule, 0, excerpt))

    found += gc.hygiene(title, cfg, kind="brief")
    found += gc.hygiene(body, cfg, kind="brief")
    if not title.strip() or len(title) > 256:
        err("the title must be 1–256 characters")
    try:
        meta = draft_meta(body)
    except gc.CollabError as e:
        err(str(e))
        return found
    task = str(meta.get("task") or "")
    if not gc.SLUG_RE.fullmatch(str(meta.get("slug", ""))):
        err("the brief marker's slug is missing or not a plain branch-name part")
    if meta.get("base") != cfg["base_branch"]:
        warn(f"the marker's base {meta.get('base')!r} is not the configured {cfg['base_branch']!r}")
    try:
        sha = base_commit(ctx, str(meta.get("base_commit") or ""), fetch)
        if not meta.get("base_commit"):
            err("the brief marker names no base commit")
    except gc.CollabError as e:
        err(str(e))
        sha = ""
    if task:
        try:
            plan_of(task, cfg)
        except gc.CollabError as e:
            err(str(e))
        if task not in title:
            warn(f"the title does not name the task {task}")
    headings = [h.lower() for h in re.findall(r"^#{2,4}\s+(.+)$", body, re.M)]
    for section in cfg["brief"].get("required_sections", []):
        if not any(section.lower() in h for h in headings):
            err(f"no section heading names {section!r}")
    trailer = cfg["commit"].get("trailer")
    if trailer and trailer not in body:
        err("the brief does not quote the commit trailer", trailer)
    if f"{cfg['remote']}/{cfg['base_branch']}" not in body:
        warn(f"the brief never names `{cfg['remote']}/{cfg['base_branch']}`")
    if sha:
        top = set(git("ls-tree", "--name-only", sha).splitlines())
        for path in sorted(set(TOP_LEVEL_PATH_RE.findall(body))):
            if path.split("/", 1)[0] in top and not git.ok("cat-file", "-e", f"{sha}:{path.rstrip('/')}"):
                warn(f"`{path}` does not exist at {sha}")
    # Labels and milestone.
    existing = repo_labels(gh)
    state = set(gc.state_labels(cfg))
    for label in labels:
        if label in state:
            err(f"a new brief can't carry the state label {label!r}")
        elif label not in existing:
            err(f"no label {label!r} in {gh.repo}")
    for family in cfg["brief"].get("label_families", []):
        if any(lab.startswith(family) for lab in existing) and not any(lab.startswith(family) for lab in labels):
            warn(f"no {family}* label chosen")
    if milestone:
        if milestone not in [m.get("title") for m in open_milestones(gh)]:
            err(f"no open milestone titled {milestone!r}")
    elif cfg["brief"].get("require_milestone"):
        err("no milestone chosen (--milestone)")
    if not allow_duplicate:
        for issue in gc.list_task_issues(gh, cfg):
            other = gc.brief_meta(issue, cfg) if issue.body else {}
            if task and (re.search(r"(?<![\w.])" + re.escape(task) + r"(?![\w])", issue.title)
                         or other.get("task") == task):
                err(f"#{issue.number} already briefs {task}: pass --allow-duplicate if intended", issue.title)
    return found


def print_findings(found: List[gc.Finding]) -> int:
    for finding in found:
        print(finding)
    bad = len(gc.errors(found))
    print("check: clean" if not found else f"check: {bad} error(s), {len(found) - bad} warning(s)")
    return gc.EXIT_FAIL if bad else gc.EXIT_OK


def post(ctx: gc.Ctx, body: str, title: str, labels: List[str], milestone: str, dry_run: bool) -> int:
    cfg, gh = ctx.cfg, ctx.gh
    all_labels = [cfg["labels"]["task"]] + [lab for lab in labels if lab != cfg["labels"]["task"]]
    argv = ["issue", "create", "--repo", gh.repo, "--title", title, "--body-file", "-",
            "--label", ",".join(all_labels)] + (["--milestone", milestone] if milestone else [])
    board = gc.Board(cfg, gh)
    if dry_run:
        ok, why = board.usable()
        print("DRY RUN: nothing posted.")
        print("would run: gh " + shlex.join(argv) + f"  (body: {len(body)} characters on stdin)")
        print("then board: " + (f"add as {cfg['project']['statuses']['queued']}" if ok else f"skipped ({why})"))
        print(f"title: {title}\nlabels: {', '.join(all_labels)}\nmilestone: {milestone or '(none)'}")
        print("----- body -----")
        print(body)
        return gc.EXIT_OK
    url = gh.call(*argv, stdin=body).out.strip().splitlines()[-1]
    print(f"posted: {url}")
    print(gc.board_update(cfg, gh, url, "queued", board))
    return gc.EXIT_OK


def answer(ctx: gc.Ctx, number: int, text: str, requeue: bool, dry_run: bool) -> int:
    cfg, gh = ctx.cfg, ctx.gh
    issue = gc.fetch_issue(gh, number)
    gc.require_trusted(issue, cfg)
    # The worker reads only the comments that begin with the orchestrator's tag.
    body = gc.tagged(cfg, "orchestrator", text) + "\n\n" + gc.make_marker("answer", {"requeue": requeue})
    bad = gc.errors(gc.hygiene(body, cfg, kind="brief"))  # an answer amends the brief
    if bad:
        return print_findings(bad)
    remove = [lab for lab in gc.state_labels(cfg) if issue.has(lab)]
    if dry_run:
        print(f"DRY RUN: nothing posted. Would comment on #{number}"
              + (f" and remove {', '.join(remove) or 'no labels'}" if requeue else "") + ":")
        print(body)
        return gc.EXIT_OK
    gh.call("issue", "comment", str(number), "--repo", gh.repo, "--body-file", "-", stdin=body)
    print(f"commented on #{number}")
    if requeue:
        if remove:
            args = ["issue", "edit", str(number), "--repo", gh.repo]
            for lab in remove:
                args += ["--remove-label", lab]
            gh.call(*args)
        print(f"requeued #{number}")
        print(gc.board_update(cfg, gh, issue.url, "queued"))
    return gc.EXIT_OK


LABEL_COLOURS = {"task": "1d76db", "claimed": "fbca04", "done": "0e8a16", "blocked": "b60205"}
LABEL_TEXT = {"task": "A task brief for the issue-working agent (allowed authors only)",
              "claimed": "Claimed by the issue-working agent", "done": "Done; lane branch pushed, report in comments",
              "blocked": "Blocked; question in comments"}


def setup_labels(ctx: gc.Ctx, dry_run: bool) -> int:
    existing = set(repo_labels(ctx.gh))
    missing = [k for k in LABEL_COLOURS if ctx.cfg["labels"][k] not in existing]
    for key in missing:
        name = ctx.cfg["labels"][key]
        if dry_run:
            print(f"DRY RUN: would create label {name!r}")
        else:
            ctx.gh.call("label", "create", name, "--repo", ctx.gh.repo, "--color", LABEL_COLOURS[key],
                        "--description", LABEL_TEXT[key])
            print(f"created label {name!r}")
    if not missing:
        print("labels: all four present")
    return gc.EXIT_OK


BOARD_STEPS = """By hand, once, in the board's web page (the API can't create or change views):
  - Settings > {field}: the options {options} (add any missing one);
  - a view grouped by milestone: Layout Board, Column by {field}, Group by Milestone. The default
    table view already shows the Milestone column."""


def board_setup(ctx: gc.Ctx, dry_run: bool) -> int:
    cfg, gh = ctx.cfg, ctx.gh
    pcfg = cfg["project"]
    board = gc.Board(cfg, gh)
    ok, why = board.usable()
    steps = BOARD_STEPS.format(field=pcfg["status_field"], options=", ".join(board.wanted_options()))
    if not ok:
        print(f"board: skipped ({why}).")
        if why == gc.SCOPE_HINT:
            print("After `gh auth refresh -s project`, run board-setup again.\n" + steps)
        return gc.EXIT_OK
    proj = board.project(required=False)
    if proj is None:
        if pcfg.get("number"):
            raise gc.CollabError(f"project number {pcfg['number']} not found for {board.owner}")
        if dry_run:
            print(f"DRY RUN: would create the project {pcfg['title']!r} for {board.owner} and link it to {gh.repo}")
            print(steps)
            return gc.EXIT_OK
        proj = gh.json("project", "create", "--owner", board.owner, "--title", pcfg["title"], "--format", "json")
        board._project = proj
        gh.call("project", "link", str(proj["number"]), "--owner", board.owner, "--repo", gh.repo)
        print(f"created project #{proj['number']} {pcfg['title']!r} and linked it to {gh.repo}")
    else:
        print(f"board: project #{proj['number']} {proj.get('title', '')!r} ({proj.get('url', '')})")
    missing = board.missing_options()
    print("status options: all present" if not missing else f"status options missing: {', '.join(missing)}")
    print(steps)
    return gc.EXIT_OK


def board_sync(ctx: gc.Ctx, dry_run: bool) -> int:
    cfg, gh = ctx.cfg, ctx.gh
    board = gc.Board(cfg, gh)
    ok, why = board.usable()
    if not ok:
        print(f"board: skipped ({why})")
        return gc.EXIT_OK
    items = board.items()
    field = cfg["project"]["status_field"].lower()
    moved = 0
    for issue in gc.list_task_issues(gh, cfg):
        if gc.trust_check(issue, cfg):
            continue
        want = cfg["project"]["statuses"][gc.board_key(issue, cfg)]
        have = (items.get(issue.url) or {}).get(field)
        if have == want:
            continue
        moved += 1
        if dry_run:
            print(f"DRY RUN: #{issue.number} {have or '(not on the board)'} -> {want}")
        else:
            print(f"#{issue.number}: " + gc.board_update(cfg, gh, issue.url, gc.board_key(issue, cfg), board))
    print(f"board-sync: {moved} card(s) {'to move' if dry_run else 'moved'}")
    return gc.EXIT_OK


def read_text(path: str) -> str:
    return gc.read_body(path)


def main(argv: Optional[Sequence[str]] = None, ctx: Optional[gc.Ctx] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    sc = sub.add_parser("scaffold")
    sc.add_argument("--title", required=True)
    sc.add_argument("--slug", required=True)
    sc.add_argument("--task", default="")
    sc.add_argument("--base-commit", default="")
    sc.add_argument("--scope", action="append", default=[])
    sc.add_argument("--no-fetch", action="store_true")
    lb = sub.add_parser("labels")
    lb.add_argument("--task", default="")
    for name in ("check", "post"):
        p = sub.add_parser(name)
        p.add_argument("draft")
        p.add_argument("--title", required=True)
        p.add_argument("--label", action="append", default=[])
        p.add_argument("--milestone", default="")
        p.add_argument("--allow-duplicate", action="store_true")
        p.add_argument("--no-fetch", action="store_true")
        # For a draft written by hand: the brief marker's fields.
        p.add_argument("--slug", default="")
        p.add_argument("--task", default="")
        p.add_argument("--base-commit", default="")
        p.add_argument("--scope", action="append", default=[])
        if name == "post":
            p.add_argument("--dry-run", action="store_true")
    an = sub.add_parser("answer")
    an.add_argument("number", type=int)
    an.add_argument("--body-file", required=True)
    an.add_argument("--requeue", action="store_true")
    an.add_argument("--dry-run", action="store_true")
    for name in ("setup-labels", "board-setup", "board-sync"):
        sub.add_parser(name).add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    def body() -> int:
        c = ctx or gc.make_ctx()
        if args.cmd == "scaffold":
            print(scaffold(c, args.title, args.slug, args.task, args.base_commit, args.scope, not args.no_fetch), end="")
            return gc.EXIT_OK
        if args.cmd == "labels":
            labels = repo_labels(c.gh)
            milestones = open_milestones(c.gh)
            for family in c.cfg["brief"].get("label_families", []) + ["other"]:
                names = [lab for lab in labels if (lab.startswith(family) if family != "other" else
                         not any(lab.startswith(f) for f in c.cfg["brief"].get("label_families", [])))]
                print(f"{family}*: {', '.join(names)}" if family != "other" else f"other labels: {', '.join(names)}")
            print("open milestones:")
            for m in milestones:
                print(f"  {m.get('title')}  ({m.get('open_issues', 0)} open)")
            if args.task:
                s = suggest(c.cfg, args.task, labels, milestones)
                print(f"suggested for {args.task}: label {s['plan_label'] or '(no plan label)'}; milestone "
                      + (" or ".join(repr(t) for t in s["milestones"]) or "(none matches: choose one)"))
            return gc.EXIT_OK
        if args.cmd in ("check", "post"):
            text = with_marker(c, read_text(args.draft), args.slug, args.task, args.base_commit, args.scope,
                               not args.no_fetch)
            found = check(c, text, args.title, args.label, args.milestone, args.allow_duplicate, not args.no_fetch)
            status = print_findings(found)
            if status or args.cmd == "check":
                return status
            return post(c, text, args.title, args.label, args.milestone, args.dry_run)
        if args.cmd == "answer":
            return answer(c, args.number, read_text(args.body_file), args.requeue, args.dry_run)
        if args.cmd == "setup-labels":
            return setup_labels(c, args.dry_run)
        if args.cmd == "board-setup":
            return board_setup(c, args.dry_run)
        return board_sync(c, args.dry_run)

    return gc.main_guard(body)


if __name__ == "__main__":
    sys.exit(main())
