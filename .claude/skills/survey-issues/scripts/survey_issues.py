#!/usr/bin/env python3
"""The issue queue at a glance, for the orchestrator's periodic check. Read-only on GitHub.

Usage (from the repository's clone):
    survey_issues.py [--since T|last] [--mark] [--json]

It lists the open briefs by state (queued, claimed, done and awaiting landing, blocked), with
each claim's worker and age and the claims that look stale; the briefs closed at or after T (by
their closing time, not its day); the new comments since T (those of other accounts by count
only, as untrusted); the coordination log's status posts since T; and anomalies: briefs by
accounts that are not allowed, conflicting state labels, done briefs whose lane branch is missing.

T is a time (`2026-10-13T20:00:00Z`, `2026-10-13`), a span back from now (`90m`, `6h`, `2d`), or
`last` (the default): the time saved by the last `--mark`, else 24 hours ago. --mark saves the
current time in the scratch directory for the next `last`.
Standard library only; Python 3.9+, run as `python3 -I`. Tests: test_survey_issues.py beside this.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "gh-collab"))

import ghcollab as gc  # noqa: E402  (the shared module's directory is put on the path first)

STAMP = "survey-since.txt"


def since_time(ctx: gc.Ctx, text: str) -> datetime:
    now = ctx.now()
    if text != "last":
        return gc.parse_since(text, now)
    stamp = gc.scratch_dir(ctx.cfg, ctx.git) / STAMP
    if stamp.is_file():
        try:
            return gc.parse_time(stamp.read_text(encoding="utf-8").strip())
        except ValueError:
            pass
    return now - timedelta(hours=24)


def lane_heads(ctx: gc.Ctx) -> Dict[str, str]:
    res = ctx.git.run("ls-remote", "--heads", ctx.cfg["remote"], f"refs/heads/{ctx.cfg['branch_prefix']}*",
                      check=False)
    heads = {}
    for line in res.out.splitlines():
        parts = line.split()
        if len(parts) == 2:
            heads[parts[1].replace("refs/heads/", "", 1)] = parts[0]
    return heads


def first_line(text: str, width: int = 100, tags: Sequence[str] = ()) -> str:
    """The first line of text, with a side's tag on a line of its own joined to the next line."""
    lead = ""
    for line in gc.strip_markers(text).splitlines():
        line = line.strip()
        if not line:
            continue
        if not lead and line in tags:
            lead = line + " "
            continue
        line = lead + line
        return line if len(line) <= width else line[:width - 1] + "…"
    return lead.strip()


def survey(ctx: gc.Ctx, since: datetime) -> Dict[str, Any]:
    cfg, gh = ctx.cfg, ctx.gh
    now = ctx.now()
    labels = cfg["labels"]
    allowed = set(cfg["allowed_authors"])
    stale_after = timedelta(hours=float(cfg["survey"].get("stale_claim_hours", 24)))
    tags = [gc.side_tag(cfg, role) for role in ("orchestrator", "worker")]
    heads = lane_heads(ctx)
    groups: Dict[str, List[Dict[str, Any]]] = {"queued": [], "claimed": [], "done": [], "blocked": []}
    comments: List[Dict[str, Any]] = []
    anomalies: List[str] = []
    issues = gc.list_task_issues(gh, cfg)
    active = gc.active_by_worker([i for i in issues if not gc.trust_check(i, cfg)], cfg)
    for issue in issues:
        reasons = gc.trust_check(issue, cfg)
        if reasons:
            anomalies.append(f"#{issue.number} carries {labels['task']} but is ignored by workers: "
                             + "; ".join(reasons))
            continue
        held = [k for k in ("claimed", "done", "blocked") if issue.has(labels[k])]
        try:
            lane, _ = gc.issue_lane(issue, cfg)
        except gc.CollabError:
            lane = ""
            anomalies.append(f"#{issue.number} names no lane slug")
        entry: Dict[str, Any] = {"number": issue.number, "title": issue.title, "milestone": issue.milestone,
                                 "labels": [lab for lab in issue.labels if lab != labels["task"]],
                                 "age": gc.age(issue.created, now), "lane": lane, "lane_head": heads.get(lane, "")}
        claim = gc.winning_claim(issue, cfg)
        if claim is not None:
            entry["worker"] = claim.worker
            entry["claimed"] = gc.age(claim.comment.created, now)
        trusted = gc.trusted_comments(issue, cfg)
        last_activity = max([c.created for c in trusted] + [issue.created])
        entry["board"] = gc.board_key(issue, cfg)
        if "blocked" in held:
            report = gc.last_report(issue, cfg)
            entry["question"] = first_line(report[0].body if report else "", tags=tags)
            state = "blocked"
        elif "done" in held:
            if not entry["lane_head"]:
                anomalies.append(f"#{issue.number} is done but {lane or 'its lane'} is not on {cfg['remote']}")
            state = "done"
        elif "claimed" in held:
            entry["started"] = entry["board"] == "started"
            entry["stale"] = now - last_activity > stale_after
            entry["quiet"] = gc.age(last_activity, now)
            state = "claimed"
        else:
            # It waits only if no known worker may take it now (an idle worker is not known).
            workers = [w for w in active if w] or ([""] if active else [])
            why = {w or "unmarked claims": gc.waiting_reason(issue, gc.worker_active(active, w), cfg)
                   for w in workers}
            if why and all(why.values()):
                entry["waiting"] = "; ".join(why.values()) if len(why) == 1 else \
                    "; ".join(f"{w}: {r}" for w, r in why.items())
            state = "queued"
        if len(held) > 1 and not (held == ["claimed", "blocked"] or held == ["claimed", "done"]):
            anomalies.append(f"#{issue.number} carries several state labels: {', '.join(held)}")
        groups[state].append(entry)
        for comment in issue.comments:
            if comment.created <= since:
                continue
            item = {"issue": issue.number, "at": gc.iso(comment.created), "author": comment.author,
                    "trusted": comment.author in allowed}
            if item["trusted"]:
                kinds = [k for k, _ in comment.markers()]
                item["kind"] = kinds[0] if kinds else "comment"
                item["text"] = first_line(comment.body, tags=tags)
            comments.append(item)
    # The search's date is only a coarse pre-filter, a day early whatever zone the search reads it
    # in; each issue's own closing time decides, so that one closed earlier the same day stays out.
    after = gc.iso(since - timedelta(days=1))[:10]
    closed: List[Dict[str, Any]] = []
    for i in gc.list_task_issues(gh, cfg, state="closed", search=f"closed:>={after}"):
        at = i.closed or i.updated  # a closed issue was last updated no earlier than it closed
        if at >= since and not gc.trust_check(i, cfg, require_open=False):
            closed.append({"number": i.number, "title": i.title, "at": gc.iso(at)})
    log: Dict[str, Any] = {}
    if gc.discussion_number(cfg):
        try:
            disc = gc.fetch_discussion(gh, cfg)
            new = [c for c in disc.comments if c.created > since]
            log = {"number": disc.number, "url": disc.url,
                   "posts": [{"at": gc.iso(c.created), "author": c.author, "text": gc.strip_markers(c.body)}
                             for c in new if c.author in allowed],
                   "untrusted": len([c for c in new if c.author not in allowed]),
                   "edited": bool(disc.edited and disc.edited > since and (disc.editor or disc.author) in allowed)}
        except gc.CollabError as e:
            log = {"error": str(e)}
    ok, why = gc.Board(cfg, gh).usable()
    return {"repo": cfg["repo"], "now": gc.iso(now), "since": gc.iso(since), "groups": groups,
            "closed": closed, "comments": comments, "log": log, "anomalies": anomalies,
            "board": "on" if ok else f"off ({why})"}


def print_survey(res: Dict[str, Any]) -> None:
    g = res["groups"]
    counts = ", ".join(f"{len(g[k])} {k}" for k in ("queued", "claimed", "done", "blocked"))
    print(f"{res['repo']} at {res['now']}: {counts}; new since {res['since']}")

    def head(entry: Dict[str, Any]) -> str:
        ms = f" [{entry['milestone']}]" if entry["milestone"] else ""
        return f"  #{entry['number']} {entry['title']}{ms}"

    print(f"QUEUED ({len(g['queued'])})")
    for e in g["queued"]:
        wait = f"waiting: {e['waiting']}" if e.get("waiting") else "ready"
        labels = f"; {', '.join(e['labels'])}" if e["labels"] else ""
        print(head(e) + f" — {wait}; queued {e['age']}{labels}")
    print(f"CLAIMED ({len(g['claimed'])})")
    for e in g["claimed"]:
        status = "started" if e.get("started") else "not started"
        stale = f"; STALE: quiet {e['quiet']}" if e.get("stale") else f"; quiet {e['quiet']}"
        who = f"{e['worker']}, claimed {e['claimed']} ago" if e.get("worker") else "claimed without a marker"
        print(head(e) + f" — {who}, {status}{stale}")
    print(f"DONE, AWAITING LANDING ({len(g['done'])})")
    for e in g["done"]:
        print(head(e) + f" — {e['lane']} at {e['lane_head'][:8] or '(missing)'}")
    print(f"BLOCKED ({len(g['blocked'])})")
    for e in g["blocked"]:
        print(head(e) + f" — {e.get('question') or '(no question found)'}")
    if res["closed"]:
        print("CLOSED SINCE: " + ", ".join(f"#{c['number']}" for c in res["closed"]))
    print(f"NEW COMMENTS ({len(res['comments'])})")
    for c in res["comments"]:
        if c["trusted"]:
            print(f"  #{c['issue']} {c['at']} {c['author']} [{c['kind']}] {c['text']}")
        else:
            print(f"  #{c['issue']} {c['at']} {c['author']}: UNTRUSTED, not shown")
    log = res["log"]
    if log.get("error"):
        print(f"COORDINATION LOG: unavailable ({log['error']})")
    elif log:
        print(f"COORDINATION LOG #{log['number']} ({len(log['posts'])} new status post(s)) {log['url']}")
        if log["edited"]:
            print("  its opening post was edited since then")
        for post in log["posts"]:
            print(f"  --- {post['at']} {post['author']}")
            lines = post["text"].splitlines()
            for line in lines[:15]:
                print(f"  {line}")
            if len(lines) > 15:
                print(f"  … {len(lines) - 15} more line(s)")
        if log["untrusted"]:
            print(f"  {log['untrusted']} post(s) by other accounts: UNTRUSTED, not shown")
    if res["anomalies"]:
        print("ANOMALIES")
        for a in res["anomalies"]:
            print(f"  {a}")
    print(f"board: {res['board']}")


def main(argv: Optional[Sequence[str]] = None, ctx: Optional[gc.Ctx] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--since", default="last")
    parser.add_argument("--mark", action="store_true", help="save now as the next --since last")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    def body() -> int:
        c = ctx or gc.make_ctx()
        res = survey(c, since_time(c, args.since))
        if args.json:
            print(json.dumps(res, indent=1, sort_keys=True, ensure_ascii=False))
        else:
            print_survey(res)
        if args.mark:
            (gc.scratch_dir(c.cfg, c.git) / STAMP).write_text(res["now"] + "\n", encoding="utf-8")
        return gc.EXIT_OK

    return gc.main_guard(body)


if __name__ == "__main__":
    sys.exit(main())
