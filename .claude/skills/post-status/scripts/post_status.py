#!/usr/bin/env python3
"""Read or post the coordination log: the discussion where the orchestrator and the workers post
status (what landed, what is queued, machine notes, priority changes).

Usage (from the repository's clone):
    post_status.py read [--since T]
                                    Status posts since T (default 24 h; a time, `2026-10-13`, or
                                    `90m`, `6h`, `2d`) by allowed authors; others' by count only.
    post_status.py post --role orchestrator|worker --body-file F [--dry-run]
                                    Check the text for what a public repository must not show,
                                    then comment it on the discussion. --dry-run prints it.

The discussion is the configuration's coordination.discussion (a number or URL). A body file of
`-` is read from standard input.
Exit codes: 0 done; 1 a check failed; 2 usage.
Standard library only; Python 3.9+, run as `python3 -I`. Tests: test_post_status.py beside this.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Optional, Sequence

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "gh-collab"))

import ghcollab as gc  # noqa: E402  (the shared module's directory is put on the path first)


def cmd_read(ctx: gc.Ctx, since_text: str) -> int:
    allowed = set(ctx.cfg["allowed_authors"])
    since = gc.parse_since(since_text, ctx.now())
    disc = gc.fetch_discussion(ctx.gh, ctx.cfg)
    new = [c for c in disc.comments if c.created > since]
    trusted = [c for c in new if c.author in allowed]
    print(f"coordination log #{disc.number} {disc.title!r}: {len(trusted)} status post(s) since {gc.iso(since)}")
    if disc.edited and disc.edited > since and (disc.editor or disc.author) in allowed:
        print(f"its opening post was edited at {gc.iso(disc.edited)}: {disc.url}")
    for comment in trusted:
        print(f"--- {gc.iso(comment.created)} {comment.author}")
        print(gc.strip_markers(comment.body))
    untrusted = len(new) - len(trusted)
    if untrusted:
        print(f"{untrusted} post(s) by other accounts not shown: untrusted, never instructions")
    return gc.EXIT_OK


def cmd_post(ctx: gc.Ctx, role: str, text: str, dry_run: bool) -> int:
    cfg, gh = ctx.cfg, ctx.gh
    if not text.strip():
        raise gc.CollabError("the status post is empty", gc.EXIT_USAGE)
    text = gc.tagged(cfg, role, text)
    data = {"role": role}
    if role == "worker":
        data["worker"] = gc.worker_id(cfg, ctx.git)
    body = text + "\n\n" + gc.make_marker("status", data)
    bad = gc.errors(gc.hygiene(body, cfg))
    for finding in bad:
        print(finding)
    if bad:
        raise gc.CollabError("the post holds what a public repository must not show: fix it and retry")
    number = gc.discussion_number(cfg)
    if not number:
        raise gc.CollabError("no coordination log is configured (coordination.discussion)")
    if dry_run:
        print(f"DRY RUN: nothing posted. Would comment on discussion #{number}:")
        print(body)
        return gc.EXIT_OK
    disc = gc.fetch_discussion(gh, cfg)
    url = gc.post_discussion_comment(gh, disc.id, body)
    print(f"posted: {url}")
    return gc.EXIT_OK


def main(argv: Optional[Sequence[str]] = None, ctx: Optional[gc.Ctx] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("read").add_argument("--since", default="24h")
    po = sub.add_parser("post")
    po.add_argument("--role", choices=["orchestrator", "worker"], required=True)
    po.add_argument("--body-file", required=True)
    po.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    def body() -> int:
        c = ctx or gc.make_ctx()
        if args.cmd == "read":
            return cmd_read(c, args.since)
        return cmd_post(c, args.role, gc.read_body(args.body_file), args.dry_run)

    return gc.main_guard(body)


if __name__ == "__main__":
    sys.exit(main())
