#!/usr/bin/env python3
"""Land a worker's lane on the integration branch: check it, cherry-pick it, gate it, and close
the issue once the integration branch is pushed. It never pushes.

Usage (from the integration branch's checkout):
    land_issue.py check N [--lane REF]
                                    Fetch the lane and check what would be picked: non-merge
                                    commits only, conventional subjects with the task ID, exactly
                                    one trailer, no session links or secrets added, no large added
                                    files, nothing outside the brief's scope. Changes nothing.
    land_issue.py pick N            Check, then cherry-pick the lane onto the integration branch,
                                    which must be checked out, clean and not behind its remote.
    land_issue.py gate run N [-- COMMAND...]
    land_issue.py gate check N      Run the gate into a log stamped with the tree it ran on, and
                                    check it shows a pass on HEAD's tree. Push in a separate step.
    land_issue.py close N [--note-file F] [--dry-run]
                                    Once every lane commit is on the pushed integration branch:
                                    comment what landed, close the issue and move its card to
                                    landed. --dry-run prints the closing comment.

Exit codes: 0 done; 1 a check failed or a step stopped; 2 usage; 3 untrusted.
Standard library only; Python 3.9+, run as `python3 -I`. Tests: test_land_issue.py beside this.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import List, Optional, Sequence, Tuple

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "gh-collab"))

import ghcollab as gc  # noqa: E402  (the shared module's directory is put on the path first)


def log_path(ctx: gc.Ctx, number: int) -> Path:
    return gc.scratch_dir(ctx.cfg, ctx.git) / f"land-{number}" / "gate.log"


def fetch_lane(ctx: gc.Ctx, lane: str, given: str) -> str:
    """Fetch the lane without `+`, so that a rewritten (force-pushed) lane fails loudly."""
    if given:
        return given
    remote = ctx.cfg["remote"]
    ref = f"refs/remotes/{remote}/{lane}"
    res = ctx.git.run("fetch", "--no-progress", remote, f"refs/heads/{lane}:{ref}", check=False)
    if res.code != 0:
        if "non-fast-forward" in res.err or "rejected" in res.err:
            raise gc.CollabError(f"{lane} was rewritten since the last fetch (a force-push?): inspect it by hand")
        raise gc.CollabError(f"can't fetch {lane} from {remote}: {res.err.strip()[:300]}")
    return ref


def target_ref(ctx: gc.Ctx) -> str:
    base = ctx.cfg["base_branch"]
    if ctx.git.current_branch() == base:
        return "HEAD"
    if ctx.git.has_ref(f"refs/heads/{base}"):
        return f"refs/heads/{base}"
    return f"{ctx.cfg['remote']}/{base}"


def examine(ctx: gc.Ctx, number: int, given_lane: str, target: str) -> Tuple[gc.Issue, str, str, gc.CommitReport]:
    cfg, git = ctx.cfg, ctx.git
    issue = gc.fetch_issue(ctx.gh, number)
    gc.require_trusted(issue, cfg)
    lane, meta = gc.issue_lane(issue, cfg)
    git.run("fetch", "--quiet", cfg["remote"], cfg["base_branch"])
    remote_base = f"{cfg['remote']}/{cfg['base_branch']}"
    lane_ref = fetch_lane(ctx, lane, given_lane)
    picks, merges = gc.lane_commits(git, target, lane_ref, exclude_ref=remote_base)
    report = gc.check_commits(git, cfg, meta, picks, merges, identity_ref=remote_base)
    # The lane must still hold the head its report named: a rewritten lane doesn't.
    reported = gc.last_report(issue, cfg)
    head = str((reported[1] if reported else {}).get("head") or "")
    if head and not given_lane:
        if not (git.has_ref(head) and git.is_ancestor(head, lane_ref)):
            report.problems.append(f"the lane no longer contains the reported head {head[:8]}: it was rewritten")
        elif git("rev-parse", lane_ref) != head:
            report.notes.append(f"the lane moved past the reported head {head[:8]}")
    if not issue.has(cfg["labels"]["done"]):
        report.notes.append(f"#{number} is not labelled {cfg['labels']['done']}")
    for label in (cfg["labels"]["blocked"], cfg["labels"]["claimed"]):
        if issue.has(label):
            report.notes.append(f"#{number} is still labelled {label}")
    sha = str(meta.get("base_commit") or "")
    if sha and git.has_ref(sha) and not git.is_ancestor(sha, lane_ref):
        report.notes.append(f"the lane does not contain the brief's base commit {sha}")
    return issue, lane, lane_ref, report


def print_report(ctx: gc.Ctx, issue: gc.Issue, lane: str, lane_ref: str, target: str,
                 report: gc.CommitReport) -> None:
    git = ctx.git
    print(f"#{issue.number} {issue.title}")
    print(f"lane {lane} at {git('rev-parse', '--short=8', lane_ref)}; onto {target} "
          f"({git('rev-parse', '--short=8', target)})")
    print(f"to pick ({len(report.picks)}):")
    for info in report.picks:
        print(f"  {info.sha[:8]} {info.subject}")
    files = report.files()
    print(f"files ({len(files)}): " + ", ".join(files[:40]) + (" …" if len(files) > 40 else ""))
    meta_scope = gc.brief_meta(issue, ctx.cfg)["scope"]
    print("scope: " + (", ".join(meta_scope) if meta_scope else "none declared: judge the files above"))
    for note in report.notes:
        print(f"note: {note}")
    for problem in report.problems:
        print(f"problem: {problem}")


def cmd_check(ctx: gc.Ctx, number: int, given_lane: str) -> int:
    target = target_ref(ctx)
    issue, lane, lane_ref, report = examine(ctx, number, given_lane, target)
    print_report(ctx, issue, lane, lane_ref, target, report)
    print("LAND CHECK: " + ("pass" if not report.problems else "FAIL"))
    return gc.EXIT_FAIL if report.problems else gc.EXIT_OK


def cmd_pick(ctx: gc.Ctx, number: int, given_lane: str) -> int:
    cfg, git = ctx.cfg, ctx.git
    base = cfg["base_branch"]
    remote_base = f"{cfg['remote']}/{base}"
    if git.current_branch() != base:
        raise gc.CollabError(f"check out {base} first: picks land only on the integration branch")
    if not git.is_clean():
        raise gc.CollabError("the working tree has uncommitted changes")
    git.run("fetch", "--quiet", cfg["remote"], base)
    if not git.is_ancestor(remote_base, "HEAD"):
        raise gc.CollabError(f"{base} is behind {remote_base}: bring it up to date first")
    issue, lane, lane_ref, report = examine(ctx, number, given_lane, "HEAD")
    print_report(ctx, issue, lane, lane_ref, "HEAD", report)
    if report.problems:
        print("LAND CHECK: FAIL; nothing picked")
        return gc.EXIT_FAIL
    before = git("rev-parse", "HEAD")
    res = git.run("cherry-pick", *[c.sha for c in report.picks], check=False)
    if res.code != 0:
        print((res.out + res.err).strip()[-2000:])
        print("cherry-pick stopped: resolve the conflict keeping both sides' intent and run "
              "`git cherry-pick --continue`, or `git cherry-pick --abort` to undo the picks")
        return gc.EXIT_FAIL
    if git("rev-list", "--merges", f"{before}..HEAD"):
        raise gc.CollabError("merge commits appeared on the integration branch: inspect it")
    new = git("rev-list", "--reverse", f"{before}..HEAD").splitlines()
    print(f"picked {len(new)} commit(s):")
    for old, sha in zip(report.picks, new):
        print(f"  {sha[:8]} <- {old.sha[:8]} {old.subject}")
    same = git("rev-parse", "HEAD^{tree}") == git("rev-parse", f"{lane_ref}^{{tree}}")
    print("tree: " + ("the same as the lane head's, which the worker's reported gate ran on" if same else
                      "differs from the lane head's: the worker's gate does not cover it"))
    push = " ".join(["git", "push"] + list(cfg["land"].get("push_args", [])) + [cfg["remote"], base])
    print(f"next: `land_issue.py gate run {number}` (in the background); `land_issue.py gate check {number}`; "
          f"then, in its own step, `{push}`; then `land_issue.py close {number}`")
    return gc.EXIT_OK


def cmd_gate(ctx: gc.Ctx, action: str, number: int, command: List[str]) -> int:
    log = log_path(ctx, number)
    if action == "check":
        problems = gc.gate_check(ctx.git, log)
        for p in problems:
            print(f"problem: {p}")
        print("gate check: " + ("pass" if not problems else "fail") + f" ({log})")
        return gc.EXIT_FAIL if problems else gc.EXIT_OK
    if command and command[0] == "--":
        command = command[1:]
    if not command:
        configured = ctx.cfg["land"].get("gate_command")
        if not configured:
            raise gc.CollabError("no land.gate_command is configured: pass the gate after --", gc.EXIT_USAGE)
        command = [gc.fill(configured, ctx.cfg)]
    print(f"gate log: {log}", flush=True)
    code = gc.gate_run(ctx.git, log, command, gc.scratch_dir(ctx.cfg, ctx.git))
    print(gc.gate_tail(log))
    return gc.EXIT_OK if code == 0 else gc.EXIT_FAIL


def landed_on_remote(ctx: gc.Ctx, lane_ref: str) -> Tuple[List[Tuple[str, str, str]], List[str]]:
    """Each lane commit with the remote integration commit that carries it, and those it lacks."""
    git = ctx.git
    remote_base = f"{ctx.cfg['remote']}/{ctx.cfg['base_branch']}"
    shas = [s for s in git("rev-list", "--reverse", "--no-merges", f"{remote_base}..{lane_ref}").splitlines() if s]
    if not shas:
        return [], []
    keys = gc.landed_keys(git, remote_base, shas)
    same = {line[1:].strip() for line in git("rev-list", "--cherry-mark", "--no-merges", "--right-only",
                                             f"{remote_base}...{lane_ref}").splitlines() if line.startswith("=")}
    landed, missing = [], []
    for sha in shas:
        key = gc.commit_key(git, sha)
        if key in keys:
            landed.append((sha, keys[key], key[1]))
        elif sha in same:
            landed.append((sha, "", key[1]))
        else:
            missing.append(sha)
    return landed, missing


def cmd_close(ctx: gc.Ctx, number: int, note: str, dry_run: bool, given_lane: str) -> int:
    cfg, gh, git = ctx.cfg, ctx.gh, ctx.git
    issue = gc.fetch_issue(gh, number)
    gc.require_trusted(issue, cfg)
    lane, _ = gc.issue_lane(issue, cfg)
    git.run("fetch", "--quiet", cfg["remote"], cfg["base_branch"])
    lane_ref = fetch_lane(ctx, lane, given_lane)
    landed, missing = landed_on_remote(ctx, lane_ref)
    base = cfg["base_branch"]
    if missing:
        raise gc.CollabError(f"{len(missing)} lane commit(s) are not on {cfg['remote']}/{base} yet "
                             f"({', '.join(s[:8] for s in missing)}): push the integration branch first, in its "
                             "own step, then close")
    if not landed:
        raise gc.CollabError(f"nothing from {lane} is on {cfg['remote']}/{base}: land it first")
    lines = [f"Landed on `{base}` and pushed:"]
    lines += [f"- `{new[:8] or '?'}` {subject} (lane `{old[:8]}`)" for old, new, subject in landed]
    log = log_path(ctx, number)
    tail = [ln for ln in (log.read_text(encoding="utf-8", errors="replace").splitlines() if log.is_file() else [])
            if ln.strip()]
    if tail:
        lines.append(f"\nIntegration gate: `{tail[-1].strip()}`.")
    if note.strip():
        lines.append("\n" + note.strip())
    lines.append(f"\nClosing. `{lane}` may be deleted.")
    body = "\n".join(lines) + "\n\n" + gc.make_marker("landed", {"commits": [new for _, new, _ in landed if new]})
    bad = gc.errors(gc.hygiene(body, cfg))
    if bad:
        for finding in bad:
            print(finding)
        raise gc.CollabError("the closing comment holds what a public repository must not show")
    if dry_run:
        print(f"DRY RUN: nothing posted. Would close #{number} as completed with:")
        print(body)
        return gc.EXIT_OK
    gh.call("issue", "close", str(number), "--repo", gh.repo, "--reason", "completed", "--comment", body)
    print(f"closed #{number}")
    print(gc.board_update(cfg, gh, issue.url, "landed"))
    return gc.EXIT_OK


def main(argv: Optional[Sequence[str]] = None, ctx: Optional[gc.Ctx] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    for name in ("check", "pick"):
        p = sub.add_parser(name)
        p.add_argument("number", type=int)
        p.add_argument("--lane", default="", help="a ref to land instead of the fetched lane branch")
    gt = sub.add_parser("gate")
    gt.add_argument("action", choices=["run", "check"])
    gt.add_argument("number", type=int)
    gt.add_argument("command", nargs=argparse.REMAINDER)
    cl = sub.add_parser("close")
    cl.add_argument("number", type=int)
    cl.add_argument("--note-file", default="")
    cl.add_argument("--lane", default="")
    cl.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    def body() -> int:
        c = ctx or gc.make_ctx()
        if args.cmd == "check":
            return cmd_check(c, args.number, args.lane)
        if args.cmd == "pick":
            return cmd_pick(c, args.number, args.lane)
        if args.cmd == "gate":
            return cmd_gate(c, args.action, args.number, args.command)
        note = Path(args.note_file).read_text(encoding="utf-8") if args.note_file else ""
        return cmd_close(c, args.number, note, args.dry_run, args.lane)

    return gc.main_guard(body)


if __name__ == "__main__":
    sys.exit(main())
