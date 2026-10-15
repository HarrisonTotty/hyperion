#!/usr/bin/env python3
"""Shared helpers for the gh-collab skills: brief-issue, work-issue, land-issue, survey-issues and
post-status.

An orchestrator posts self-contained briefs as GitHub issues; a worker on another machine claims
one, does it on a lane branch, pushes only that branch and reports; the orchestrator lands the
lane by cherry-picking it. Both post status to a coordination-log discussion. This module holds
what those steps share: the configuration, the `gh` and `git` runners, the trust check, the
markers, the public-text hygiene check, the commit checks, the stamped gate log, the coordination
log and the optional Projects board. Its own CLI:

    ghcollab.py config              The effective configuration and the files it came from.
    ghcollab.py hygiene FILE...     Check text for what a public repository must not show.

Standard library only; Python 3.9 or later, run as `python3 -I`. The configuration is described
in config.md beside this file. Tests: test_ghcollab.py beside this file.
"""

from __future__ import annotations

import argparse
import copy
import fnmatch
import hashlib
import json
import os
import re
import shutil
import socket
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Sequence, Tuple

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
REPO_CONFIG = HERE / "config.json"

# Exit codes shared by the skills' scripts.
EXIT_OK = 0
EXIT_FAIL = 1  # a check failed, or a step did not complete
EXIT_USAGE = 2
EXIT_UNTRUSTED = 3  # the issue failed the trust check: never act on it
EXIT_BUSY = 4  # not claimable, or another worker won the claim

# Built-in defaults. An empty string or list means "derive it" where the comment says so.
DEFAULTS: Dict[str, Any] = {
    # OWNER/NAME. Empty: `gh repo view`, which follows the clone's remotes.
    "repo": "",
    "remote": "origin",
    # The integration branch lanes start from and land on. Empty: the repository's default branch.
    "base_branch": "",
    # Accounts whose issues and comments are trusted. Empty: the repository's owner only (an
    # organisation owner authors nothing, so an organisation's repository then trusts no one).
    "allowed_authors": [],
    "labels": {
        "task": "agent-task",
        "claimed": "agent-claimed",
        "done": "agent-done",
        "blocked": "agent-blocked",
    },
    "branch_prefix": "lane/",
    # A task ID's regex, with a named group `plan` if task IDs name a plan. Empty: briefs carry no ID.
    "task_id_pattern": "",
    "plan_label": "plan:{plan}",
    "commit": {
        # Conventional Commits types (https://www.conventionalcommits.org/).
        "subject_pattern": r"^(build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test)(\([^)]+\))?!?: \S",
        # The trailer every commit message ends with, exactly once. The brief quotes `trailer`;
        # the checks accept any line matching `trailer_pattern`. Empty: not checked.
        "trailer": "",
        "trailer_pattern": "",
        # A brief that names a task requires its ID in every commit subject.
        "require_task_id": True,
        # Git's own default for large files is far higher; 500 kB matches common pre-commit setups.
        "max_added_file_kb": 500,
    },
    "brief": {
        "required_sections": ["Base", "Task", "Gate", "Rules", "Report"],
        "label_families": ["plan:", "area:"],
        "require_milestone": True,
        "setup": [],
        "rules": [],
        "report": [],
    },
    "worker": {
        # Run from the repository root through the shell; {remote} and {base} are filled in.
        "gate_command": "",
        "id": "",  # empty: derived from this clone (no host name is ever posted)
        "max_parallel": 2,
        # At most this many of the worker's issues with the label at once, within max_parallel:
        # a heavy issue waits for another heavy one, a light one runs beside it.
        "label_limits": {"heavy": 1},
        "poll_minutes": 10,
        # Seconds between posting a claim and re-reading the comments to see who won. Long
        # enough for both racers' comments to be listed, short against a poll interval.
        "claim_settle_seconds": 5,
        # Once the stamped gate log covers HEAD's tree, push without re-running the pre-push hook.
        "push_no_verify_after_gate": False,
        "deny_bash": [],  # extra regexes the worker's Bash guard refuses
    },
    "land": {
        "gate_command": "",
        "push_args": [],
    },
    "survey": {
        "stale_claim_hours": 24,
    },
    # The coordination log: a discussion (its number or URL) where both sides post status.
    # 0: there is none.
    "coordination": {
        "discussion": 0,
        # Each post begins with its side's tag, since both sides may post as one account.
        "tags": {"orchestrator": "[orchestrator]", "worker": "[worker]"},
    },
    # A Projects (v2) board kept in step with the labels. It needs the token's `project` scope
    # (`gh auth refresh -s project`); without it every board step is skipped with a note.
    "project": {
        "enabled": False,
        "owner": "",  # empty: the repository's owner
        "number": 0,  # 0: found by title
        "title": "",
        "status_field": "Status",
        # The status option for each state, found by name at run time.
        "statuses": {
            "queued": "Todo",  # the task label, unclaimed
            "claimed": "Claimed",  # the claimed label
            "started": "In progress",  # the worker has started its lane
            "done": "In progress",  # the done label: the lane awaits landing
            "blocked": "Blocked",  # the blocked label
            "landed": "Done",  # landed and closed
        },
    },
    "hygiene": {
        "allowed_emails": ["noreply@anthropic.com", "git@github.com"],
        "forbidden": [],  # extra regexes; keep personal ones in the local configuration
    },
    # Logs and stamps. Empty: <git common dir>/gh-collab, which git never commits.
    "scratch_dir": "",
}


class CollabError(Exception):
    """A failure the scripts report as one line and an exit code."""

    def __init__(self, message: str, code: int = EXIT_FAIL) -> None:
        super().__init__(message)
        self.code = code


# --------------------------------------------------------------------------------------------
# Running gh and git


@dataclass
class Result:
    code: int
    out: str
    err: str


Runner = Callable[[Sequence[str], Optional[str], Optional[str], Optional[Dict[str, str]]], Result]


def run_process(argv: Sequence[str], cwd: Optional[str] = None, stdin: Optional[str] = None,
                env: Optional[Dict[str, str]] = None) -> Result:
    """Run a command and capture its output as UTF-8 text."""
    if argv and argv[0] == "gh" and os.environ.get("GH_COLLAB_FORBID_GH"):
        raise CollabError(f"refusing to run gh under GH_COLLAB_FORBID_GH: {' '.join(argv)}")
    full_env = dict(os.environ)
    full_env.update({"GH_PROMPT_DISABLED": "1", "NO_COLOR": "1", "GH_NO_UPDATE_NOTIFIER": "1"})
    if env:
        full_env.update(env)
    try:
        proc = subprocess.run(list(argv), cwd=cwd, input=stdin, capture_output=True, text=True,
                              encoding="utf-8", errors="replace", env=full_env)
    except FileNotFoundError:
        raise CollabError(f"`{argv[0]}` is not on PATH")
    return Result(proc.returncode, proc.stdout, proc.stderr)


class Gh:
    """`gh` against one repository. Every call names the repository explicitly."""

    def __init__(self, repo: str = "", runner: Runner = run_process, cwd: Optional[str] = None) -> None:
        self.repo = repo
        self.runner = runner
        self.cwd = cwd

    def call(self, *args: str, stdin: Optional[str] = None, check: bool = True) -> Result:
        res = self.runner(["gh", *args], self.cwd, stdin, None)
        if check and res.code != 0:
            raise CollabError(f"gh {' '.join(args[:3])} failed (exit {res.code}): {res.err.strip()[:400]}")
        return res

    def json(self, *args: str) -> Any:
        out = self.call(*args).out
        try:
            return json.loads(out) if out.strip() else None
        except json.JSONDecodeError as e:
            raise CollabError(f"gh {' '.join(args[:3])} printed invalid JSON: {e}")


class Git:
    """`git` in one working tree."""

    def __init__(self, cwd: Optional[str] = None, runner: Runner = run_process) -> None:
        self.cwd = cwd
        self.runner = runner

    def run(self, *args: str, check: bool = True, env: Optional[Dict[str, str]] = None,
            stdin: Optional[str] = None) -> Result:
        res = self.runner(["git", *args], self.cwd, stdin, env)
        if check and res.code != 0:
            raise CollabError(f"git {' '.join(args[:4])} failed (exit {res.code}): {res.err.strip()[:400]}")
        return res

    def __call__(self, *args: str, env: Optional[Dict[str, str]] = None) -> str:
        return self.run(*args, env=env).out.strip()

    def ok(self, *args: str) -> bool:
        return self.run(*args, check=False).code == 0

    def root(self) -> Path:
        return Path(self("rev-parse", "--show-toplevel"))

    def common_dir(self) -> Path:
        path = Path(self("rev-parse", "--git-common-dir"))
        return path if path.is_absolute() else (Path(self.cwd or ".") / path).resolve()

    def is_primary_worktree(self) -> bool:
        git_dir = Path(self("rev-parse", "--absolute-git-dir")).resolve()
        return git_dir == self.common_dir().resolve()

    def current_branch(self) -> str:
        return self.run("symbolic-ref", "--quiet", "--short", "HEAD", check=False).out.strip()

    def is_clean(self) -> bool:
        return self("status", "--porcelain") == ""

    def is_ancestor(self, a: str, b: str) -> bool:
        return self.ok("merge-base", "--is-ancestor", a, b)

    def has_ref(self, ref: str) -> bool:
        return self.ok("rev-parse", "--verify", "--quiet", ref + "^{commit}")


# --------------------------------------------------------------------------------------------
# Configuration


def deep_merge(base: Dict[str, Any], over: Dict[str, Any]) -> Dict[str, Any]:
    out = copy.deepcopy(base)
    for key, value in over.items():
        if isinstance(value, dict) and isinstance(out.get(key), dict):
            out[key] = deep_merge(out[key], value)
        else:
            out[key] = copy.deepcopy(value)
    return out


def _read_json(path: Path) -> Dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as e:
        # Fail closed: a configuration that can't be read must not fall back to looser defaults.
        raise CollabError(f"gh-collab configuration {path} is unreadable: {e}")
    if not isinstance(data, dict):
        raise CollabError(f"gh-collab configuration {path} must hold a JSON object")
    return {k: v for k, v in data.items() if not k.startswith("//")}


def config_paths(git: Optional[Git]) -> List[Path]:
    """The configuration files in the order they apply: the repository's, this clone's, then
    $GH_COLLAB_CONFIG. Missing files are skipped."""
    paths = [REPO_CONFIG]
    if git is not None:
        try:
            paths.append(git.common_dir() / "gh-collab" / "config.json")
        except CollabError:
            pass
    env_path = os.environ.get("GH_COLLAB_CONFIG")
    if env_path:
        paths.append(Path(env_path))
    return paths


def load_config(git: Optional[Git] = None, paths: Optional[List[Path]] = None) -> Dict[str, Any]:
    cfg = copy.deepcopy(DEFAULTS)
    sources = ["built-in defaults"]
    for path in (paths if paths is not None else config_paths(git)):
        if path.is_file():
            cfg = deep_merge(cfg, _read_json(path))
            sources.append(str(path))
    cfg["_sources"] = sources
    validate_config(cfg)
    return cfg


def validate_config(cfg: Dict[str, Any]) -> None:
    def need(cond: bool, what: str) -> None:
        if not cond:
            raise CollabError(f"gh-collab configuration: {what}")

    need(isinstance(cfg.get("allowed_authors"), list)
         and all(isinstance(a, str) and a for a in cfg["allowed_authors"]),
         "allowed_authors must be a list of account names")
    labels = cfg.get("labels")
    need(isinstance(labels, dict) and all(isinstance(labels.get(k), str) and labels.get(k)
                                          for k in ("task", "claimed", "done", "blocked")),
         "labels needs non-empty task, claimed, done and blocked names")
    need(len({labels[k] for k in ("task", "claimed", "done", "blocked")}) == 4, "the four labels must differ")
    prefix = cfg.get("branch_prefix")
    need(isinstance(prefix, str) and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*/", prefix or "") is not None,
         "branch_prefix must be a simple name ending in '/', such as 'lane/'")
    for key in ("subject_pattern", "trailer_pattern"):
        try:
            re.compile(cfg["commit"].get(key) or "")
        except re.error as e:
            raise CollabError(f"gh-collab configuration: commit.{key} is not a regex: {e}")
    if cfg.get("task_id_pattern"):
        try:
            re.compile(cfg["task_id_pattern"])
        except re.error as e:
            raise CollabError(f"gh-collab configuration: task_id_pattern is not a regex: {e}")
    limits = cfg["worker"].get("label_limits")
    need(isinstance(limits, dict) and all(isinstance(v, int) and v >= 0 for v in limits.values()),
         "worker.label_limits must map labels to whole numbers")
    for pattern in cfg["worker"].get("deny_bash", []) + cfg["hygiene"].get("forbidden", []):
        try:
            re.compile(pattern)
        except re.error as e:
            raise CollabError(f"gh-collab configuration: bad regex {pattern!r}: {e}")


def resolve_repo(cfg: Dict[str, Any], gh: Gh) -> Dict[str, Any]:
    """Fill the repository, its base branch and the allowed authors from `gh repo view` where the
    configuration leaves them empty."""
    if cfg["repo"] and cfg["base_branch"] and cfg["allowed_authors"]:
        gh.repo = cfg["repo"]
        return cfg
    args = ["repo", "view"] + ([cfg["repo"]] if cfg["repo"] else []) + [
        "--json", "nameWithOwner,owner,defaultBranchRef"]
    info = gh.json(*args) or {}
    cfg = dict(cfg)
    cfg["repo"] = cfg["repo"] or info.get("nameWithOwner", "")
    cfg["base_branch"] = cfg["base_branch"] or (info.get("defaultBranchRef") or {}).get("name", "")
    if not cfg["allowed_authors"]:
        owner = (info.get("owner") or {}).get("login")
        cfg["allowed_authors"] = [owner] if owner else []
    if not cfg["repo"] or not cfg["base_branch"]:
        raise CollabError("could not resolve the repository or its base branch: set repo and base_branch")
    gh.repo = cfg["repo"]
    return cfg


def scratch_dir(cfg: Dict[str, Any], git: Git) -> Path:
    path = Path(cfg["scratch_dir"]) if cfg.get("scratch_dir") else git.common_dir() / "gh-collab"
    path.mkdir(parents=True, exist_ok=True)
    return path


def worker_id(cfg: Dict[str, Any], git: Git) -> str:
    """This clone's worker name: configured, or a hash of the host and clone, which reveals neither."""
    if cfg["worker"].get("id"):
        return str(cfg["worker"]["id"])
    seed = f"{socket.gethostname()}:{git.common_dir()}"
    return "w-" + hashlib.sha256(seed.encode("utf-8")).hexdigest()[:8]


def fill(template: str, cfg: Dict[str, Any]) -> str:
    """Fill {remote} and {base} in a configured command."""
    return template.replace("{remote}", cfg["remote"]).replace("{base}", cfg["base_branch"])


# --------------------------------------------------------------------------------------------
# Time


def parse_time(text: str) -> datetime:
    """GitHub's `2026-10-13T21:07:07Z` (Python 3.9's fromisoformat refuses the Z)."""
    text = text.strip()
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    value = datetime.fromisoformat(text)
    return value if value.tzinfo else value.replace(tzinfo=timezone.utc)


def now_utc() -> datetime:
    return datetime.now(timezone.utc).replace(microsecond=0)


def iso(value: datetime) -> str:
    return value.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def age(value: datetime, now: datetime) -> str:
    minutes = max(0, int((now - value).total_seconds() // 60))
    if minutes < 120:
        return f"{minutes} min"
    hours = minutes // 60
    return f"{hours} h" if hours < 48 else f"{hours // 24} d"


def parse_since(text: str, now: datetime) -> datetime:
    """`2026-10-13T20:00:00Z`, `2026-10-13`, or a span back from now: `90m`, `6h`, `2d`."""
    m = re.fullmatch(r"(\d+)([mhd])", text.strip())
    if m:
        unit = {"m": "minutes", "h": "hours", "d": "days"}[m.group(2)]
        return now - timedelta(**{unit: int(m.group(1))})
    if re.fullmatch(r"\d{4}-\d{2}-\d{2}", text.strip()):
        return parse_time(text.strip() + "T00:00:00Z")
    return parse_time(text)


# --------------------------------------------------------------------------------------------
# Markers: hidden, machine-readable notes in issue bodies and comments


MARKER_RE = re.compile(r"<!--\s*gh-collab:([a-z][a-z-]*)\s+(\{.*?\})\s*-->", re.S)


def make_marker(kind: str, data: Dict[str, Any]) -> str:
    # `>` is escaped so that no value can close the HTML comment early.
    payload = json.dumps(data, sort_keys=True, ensure_ascii=False).replace(">", "\\u003e")
    return f"<!-- gh-collab:{kind} {payload} -->"


def find_markers(text: str) -> List[Tuple[str, Dict[str, Any]]]:
    found = []
    for m in MARKER_RE.finditer(text or ""):
        try:
            data = json.loads(m.group(2))
        except json.JSONDecodeError:
            continue
        if isinstance(data, dict):
            found.append((m.group(1), data))
    return found


def strip_markers(text: str) -> str:
    return MARKER_RE.sub("", text or "").strip()


# --------------------------------------------------------------------------------------------
# Issues and the trust check


@dataclass
class Comment:
    id: int  # the REST id, increasing with time: the tie-break between equal timestamps
    node_id: str
    author: str
    body: str
    created: datetime
    url: str

    def markers(self) -> List[Tuple[str, Dict[str, Any]]]:
        return find_markers(self.body)


@dataclass
class Issue:
    number: int
    title: str
    state: str
    author: str
    labels: List[str]
    body: str
    comments: List[Comment]
    milestone: str
    created: datetime
    updated: datetime
    url: str
    closed: Optional[datetime] = None  # when it was closed; None while open, or if not listed
    editor: Optional[str] = None  # the last account to edit the body; None if never edited
    editor_known: bool = False  # whether the editor was looked up
    edited: bool = False  # whether the body was ever edited (its editor may have been deleted)

    def has(self, label: str) -> bool:
        return label in self.labels


ISSUE_FIELDS = "number,title,state,author,labels,body,comments,milestone,createdAt,updatedAt,url"
LIST_FIELDS = "number,title,state,author,labels,body,milestone,createdAt,updatedAt,closedAt,url,comments"


def _comment_from(raw: Dict[str, Any]) -> Comment:
    url = raw.get("url") or ""
    m = re.search(r"issuecomment-(\d+)", url)
    return Comment(
        id=int(m.group(1)) if m else 0,
        node_id=str(raw.get("id") or ""),
        author=((raw.get("author") or {}).get("login") or ""),
        body=raw.get("body") or "",
        created=parse_time(raw.get("createdAt") or "1970-01-01T00:00:00Z"),
        url=url,
    )


def issue_from(raw: Dict[str, Any]) -> Issue:
    comments = sorted((_comment_from(c) for c in raw.get("comments") or []), key=lambda c: (c.created, c.id))
    return Issue(
        number=int(raw["number"]),
        title=raw.get("title") or "",
        state=(raw.get("state") or "").upper(),
        author=((raw.get("author") or {}).get("login") or ""),
        labels=[lab.get("name", "") for lab in raw.get("labels") or []],
        body=raw.get("body") or "",
        comments=comments,
        milestone=((raw.get("milestone") or {}).get("title") or ""),
        created=parse_time(raw.get("createdAt") or "1970-01-01T00:00:00Z"),
        updated=parse_time(raw.get("updatedAt") or "1970-01-01T00:00:00Z"),
        url=raw.get("url") or "",
        closed=parse_time(raw["closedAt"]) if raw.get("closedAt") else None,
    )


EDITOR_QUERY = ("query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name)"
                "{issue(number:$number){editor{login} lastEditedAt}}}")


def fetch_issue(gh: Gh, number: int, with_editor: bool = True) -> Issue:
    raw = gh.json("issue", "view", str(number), "--repo", gh.repo, "--json", ISSUE_FIELDS)
    if not raw:
        raise CollabError(f"issue #{number} not found in {gh.repo}")
    issue = issue_from(raw)
    if with_editor:
        owner, name = gh.repo.split("/", 1)
        data = gh.json("api", "graphql", "-f", f"query={EDITOR_QUERY}", "-f", f"owner={owner}",
                       "-f", f"name={name}", "-F", f"number={number}")
        node = (((data or {}).get("data") or {}).get("repository") or {}).get("issue") or {}
        issue.editor = (node.get("editor") or {}).get("login")
        issue.edited = bool(node.get("lastEditedAt"))
        issue.editor_known = True
    return issue


def list_task_issues(gh: Gh, cfg: Dict[str, Any], state: str = "open", search: str = "") -> List[Issue]:
    args = ["issue", "list", "--repo", gh.repo, "--label", cfg["labels"]["task"], "--state", state,
            "--limit", "200", "--json", LIST_FIELDS]
    if search:
        args += ["--search", search]
    raw = gh.json(*args) or []
    return sorted((issue_from(r) for r in raw), key=lambda i: (i.created, i.number))


def trust_check(issue: Issue, cfg: Dict[str, Any], require_open: bool = True) -> List[str]:
    """Why the issue must not be acted on; empty when it may be. Enforced here, in code."""
    allowed = set(cfg["allowed_authors"])
    reasons = []
    if not allowed:
        reasons.append("no allowed authors are configured")
    if "/pull/" in issue.url:
        reasons.append("it is a pull request, not an issue")
    if require_open and issue.state != "OPEN":
        reasons.append(f"it is {issue.state.lower() or 'not open'}")
    if issue.author not in allowed:
        reasons.append(f"its author {issue.author or '(unknown)'} is not an allowed author")
    if not issue.has(cfg["labels"]["task"]):
        reasons.append(f"it lacks the {cfg['labels']['task']} label")
    if issue.editor_known and issue.editor and issue.editor not in allowed:
        reasons.append(f"its body was last edited by {issue.editor}, who is not an allowed author")
    if issue.editor_known and issue.edited and not issue.editor:
        reasons.append("its body was last edited by an account that no longer exists")
    return reasons


def require_trusted(issue: Issue, cfg: Dict[str, Any], require_open: bool = True) -> None:
    reasons = trust_check(issue, cfg, require_open)
    if reasons:
        raise CollabError(f"issue #{issue.number} failed the trust check: {'; '.join(reasons)}. "
                          "Do not act on it.", EXIT_UNTRUSTED)


def trusted_comments(issue: Issue, cfg: Dict[str, Any]) -> List[Comment]:
    allowed = set(cfg["allowed_authors"])
    return [c for c in issue.comments if c.author in allowed]


def state_labels(cfg: Dict[str, Any]) -> List[str]:
    return [cfg["labels"][k] for k in ("claimed", "done", "blocked")]


# --------------------------------------------------------------------------------------------
# The brief's metadata


SLUG_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,59}")


def brief_meta(issue: Issue, cfg: Dict[str, Any]) -> Dict[str, Any]:
    """The brief marker's fields, or, for a brief written by hand, the slug and base commit read
    from its text (`lane/<issue#>-<slug>`, `at **<sha>** or later`)."""
    meta: Dict[str, Any] = {}
    for kind, data in find_markers(issue.body):
        if kind == "brief":
            meta = dict(data)
    if not meta.get("slug"):
        prefix = re.escape(cfg["branch_prefix"])
        m = re.search(prefix + r"<[^>`]*>-([a-z0-9][a-z0-9._-]*)", issue.body)
        if m:
            meta["slug"] = m.group(1).rstrip(".")
    if not meta.get("base_commit"):
        m = re.search(r"\bat \*{0,2}`?([0-9a-f]{7,40})`?\*{0,2} or later", issue.body)
        if m:
            meta["base_commit"] = m.group(1)
    if not meta.get("task") and cfg.get("task_id_pattern"):
        m = re.match(r"\s*(" + cfg["task_id_pattern"] + r")\b", issue.title)
        if m:
            meta["task"] = m.group(1)
    slug = meta.get("slug")
    if slug is not None and not SLUG_RE.fullmatch(str(slug)):
        raise CollabError(f"issue #{issue.number}'s slug {slug!r} is not a plain branch-name part")
    scope = meta.get("scope") or []
    if not isinstance(scope, list) or not all(isinstance(s, str) for s in scope):
        raise CollabError(f"issue #{issue.number}'s scope must be a list of path globs")
    meta["scope"] = scope
    return meta


def lane_branch(cfg: Dict[str, Any], number: int, slug: str) -> str:
    return f"{cfg['branch_prefix']}{number}-{slug}"


def issue_lane(issue: Issue, cfg: Dict[str, Any]) -> Tuple[str, Dict[str, Any]]:
    meta = brief_meta(issue, cfg)
    if not meta.get("slug"):
        raise CollabError(f"issue #{issue.number} names no lane slug: add a brief marker "
                          f"(brief-issue's scaffold) or `{cfg['branch_prefix']}<issue#>-<slug>` to it")
    return lane_branch(cfg, issue.number, meta["slug"]), meta


# --------------------------------------------------------------------------------------------
# Claims


@dataclass
class Claim:
    worker: str
    nonce: str
    comment: Comment


def current_claims(issue: Issue, cfg: Dict[str, Any]) -> List[Claim]:
    """Trusted claim markers posted since the last trusted report, release or requeue, earliest
    first."""
    claims: List[Claim] = []
    for comment in trusted_comments(issue, cfg):
        for kind, data in comment.markers():
            if kind in ("report", "release") or (kind == "answer" and data.get("requeue")):
                claims = []
            elif kind == "claim":
                claims.append(Claim(str(data.get("worker", "")), str(data.get("nonce", "")), comment))
    return sorted(claims, key=lambda c: (c.comment.created, c.comment.id))


def winning_claim(issue: Issue, cfg: Dict[str, Any]) -> Optional[Claim]:
    claims = current_claims(issue, cfg)
    return claims[0] if claims else None


def last_report(issue: Issue, cfg: Dict[str, Any]) -> Optional[Tuple[Comment, Dict[str, Any]]]:
    found = None
    for comment in trusted_comments(issue, cfg):
        for kind, data in comment.markers():
            if kind == "report":
                found = (comment, data)
    return found


def active_by_worker(issues: List[Issue], cfg: Dict[str, Any]) -> Dict[str, List[Issue]]:
    """The claimed issues still being worked (not done or blocked), by the worker that won each."""
    labels = cfg["labels"]
    active: Dict[str, List[Issue]] = {}
    for issue in issues:
        if issue.has(labels["claimed"]) and not (issue.has(labels["done"]) or issue.has(labels["blocked"])):
            claim = winning_claim(issue, cfg)
            active.setdefault(claim.worker if claim else "", []).append(issue)
    return active


def worker_active(active: Dict[str, List[Issue]], worker: str) -> List[Issue]:
    """A worker's active issues, with every claim that carries no marker (claimed by hand, so
    possibly this worker's) counted against it too."""
    return active.get(worker, []) + (active.get("", []) if worker else [])


def waiting_reason(issue: Issue, active: List[Issue], cfg: Dict[str, Any]) -> str:
    """Why a worker with these active issues must not claim this one yet; empty if it may."""

    def numbers(items: List[Issue]) -> str:
        return ", ".join(f"#{i.number}" for i in items)

    limit = int(cfg["worker"].get("max_parallel", 1))
    if len(active) >= limit:
        return f"all {limit} slots busy ({numbers(active)})"
    for label, cap in sorted((cfg["worker"].get("label_limits") or {}).items()):
        if issue.has(label):
            if int(cap) == 0:
                return f"this worker takes no {label} issues"
            holding = [i for i in active if i.has(label)]
            if len(holding) >= int(cap):
                return f"{label} slot busy ({numbers(holding)})"
    return ""


# --------------------------------------------------------------------------------------------
# Hygiene: what a public repository's issues, comments and commits must not show


@dataclass
class Finding:
    level: str  # "error" blocks; "warning" is reported
    rule: str
    line: int
    excerpt: str

    def __str__(self) -> str:
        where = f"line {self.line}" if self.line else "text"
        return f"{self.level}: {self.rule} ({where}): {self.excerpt}"


EMAIL_RE = re.compile(r"(?<![\w.+-])[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}\b")
# A link carries a session id after `session_` or `session/`; a rule that only names the pattern
# (as the briefs' rules do) carries none, and passes.
SESSION_RES = [re.compile(r"claude\.ai/code/session[_/][\w-]+", re.I), re.compile(r"^\s*Claude-Session:", re.I | re.M)]
SECRET_RES = [
    ("GitHub token", re.compile(r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,})")),
    ("Anthropic API key", re.compile(r"\bsk-ant-[A-Za-z0-9_-]{20,}")),
    ("API key", re.compile(r"\bsk-[A-Za-z0-9]{32,}\b")),
    ("AWS access key", re.compile(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b")),
    ("Google API key", re.compile(r"\bAIza[0-9A-Za-z_-]{35}\b")),
    ("Slack token", re.compile(r"\bxox[abposr]-[A-Za-z0-9-]{10,}")),
    ("private key", re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----")),
    ("secret assignment", re.compile(
        r"(?i)\b(?:api[_-]?key|secret|access[_-]?token|auth[_-]?token|password|passwd)\b\s*[:=]\s*['\"]?"
        r"[A-Za-z0-9/+_=-]{16,}")),
]
HOME_RE = re.compile(r"(?:/home/|/Users/|[A-Za-z]:\\Users\\)([A-Za-z0-9._-]+)")
GENERIC_HOMES = {"user", "username", "runner", "you", "me", "name", "example"}
PRIVATE_IP_RE = re.compile(r"\b(?:10\.\d{1,3}|192\.168|172\.(?:1[6-9]|2\d|3[01]))\.\d{1,3}\.\d{1,3}\b")
GIT_DIR_RE = re.compile(r"(?<![\w-])\.git/[\w.-]+/")
FILL_RE = re.compile(r"\[\[FILL\b")


def _line_of(text: str, index: int) -> int:
    return text.count("\n", 0, index) + 1


def _excerpt(text: str, start: int, end: int) -> str:
    return text[start:end].replace("\n", " ")[:80]


def host_patterns(hostname: str) -> List["re.Pattern[str]"]:
    """Where this machine's name may appear. A distinctive name (digits, '-', '_', mixed case or
    12+ characters, as in `Alices-MacBook-Pro`) is refused anywhere. A plain word (a machine
    named after a star) is refused only where it names a host, `name.local`, `@name` or `name:~`,
    so that prose using the word still passes."""
    host = hostname.split(".")[0]
    if len(host) < 3 or host.lower() in {"localhost", "local"}:
        return []
    esc = re.escape(host)
    mixed = host not in (host.lower(), host.upper(), host.capitalize())
    if re.search(r"[\d_-]", host) or len(host) >= 12 or mixed:
        return [re.compile(r"(?<![\w-])" + esc + r"(?![\w-])", re.I)]
    return [re.compile(r"(?<![\w-])" + esc + r"(?:\.(?:local|lan|home|internal)\b|:[~/])", re.I),
            re.compile(r"@" + esc + r"(?![\w.-])", re.I)]


def hygiene(text: str, cfg: Dict[str, Any], kind: str = "comment") -> List[Finding]:
    """Findings for text bound for the public repository. `kind` is "brief", "comment" or "diff"
    (a diff's added lines: only links and secrets are checked, since code may hold addresses)."""
    found: List[Finding] = []

    def add(level: str, rule: str, m: "re.Match[str]") -> None:
        found.append(Finding(level, rule, _line_of(text, m.start()), _excerpt(text, m.start(), m.end())))

    for rx in SESSION_RES:
        for m in rx.finditer(text):
            add("error", "Claude session link", m)
    for name, rx in SECRET_RES:
        for m in rx.finditer(text):
            add("error", f"possible secret ({name})", m)
    if kind == "diff":
        return found

    allowed = {e.lower() for e in cfg["hygiene"].get("allowed_emails", [])}
    for m in EMAIL_RE.finditer(text):
        address = m.group(0).lower()
        local, _, domain = address.partition("@")
        if address in allowed or local == "noreply" or domain.endswith("users.noreply.github.com"):
            continue
        add("error", "email address", m)
    for m in HOME_RE.finditer(text):
        if m.group(1).lower() not in GENERIC_HOMES:
            add("error", "local home path", m)
    for rx in host_patterns(socket.gethostname()):
        for m in rx.finditer(text):
            add("error", "this machine's host name", m)
    for m in PRIVATE_IP_RE.finditer(text):
        add("warning", "private network address", m)
    for pattern in cfg["hygiene"].get("forbidden", []):
        for m in re.finditer(pattern, text):
            add("error", "a configured forbidden term", m)
    if kind == "brief":
        for m in GIT_DIR_RE.finditer(text):
            add("error", "a file inside a .git directory, which the worker cannot read: paste what it needs", m)
        for m in FILL_RE.finditer(text):
            add("error", "an unfilled [[FILL]] placeholder", m)
    if len(text) > 65536:
        found.append(Finding("error", "longer than GitHub's 65,536-character limit", 0, f"{len(text)} characters"))
    return found


def errors(findings: List[Finding]) -> List[Finding]:
    return [f for f in findings if f.level == "error"]


# --------------------------------------------------------------------------------------------
# Commit checks, shared by the worker's push and the orchestrator's landing


# What a lane may never change, whatever its brief says: the collaboration's own code, its
# configuration (the allowed authors), the worker agent and the session settings that hold the
# hooks. The orchestrator changes them itself. Globs in fnmatch's sense: `*` crosses `/`.
PROTECTED = [
    ".claude/gh-collab/*",
    ".claude/skills/brief-issue/*",
    ".claude/skills/work-issue/*",
    ".claude/skills/land-issue/*",
    ".claude/skills/survey-issues/*",
    ".claude/skills/post-status/*",
    ".claude/agents/issue-worker.md",
    ".claude/settings.json",
    ".claude/settings.local.json",
]


def protected(path: str) -> bool:
    return any(fnmatch.fnmatchcase(path, glob) for glob in PROTECTED)


@dataclass
class CommitInfo:
    sha: str
    subject: str
    message: str
    author_date: str  # raw: seconds and zone, preserved by cherry-pick
    files: List[str] = field(default_factory=list)
    emails: Tuple[str, str] = ("", "")  # the author's and the committer's


def commit_info(git: Git, sha: str) -> CommitInfo:
    out = git.run("show", "-s", "--format=%H%x00%ad%x00%ae%x00%ce%x00%B", "--date=raw", sha).out
    full, date, author, committer, message = out.split("\x00", 4)
    files = [f for f in git("diff-tree", "--no-commit-id", "--name-only", "-r", "--root", sha).splitlines() if f]
    message = message.rstrip("\n")
    return CommitInfo(full.strip(), message.split("\n", 1)[0], message, date.strip(), files,
                      (author.strip(), committer.strip()))


def known_identities(git: Git, ref: str) -> set:
    """The author and committer addresses of ref's recent history: a lane may use these."""
    return {e.strip().lower() for e in git("log", "-500", "--format=%ae%n%ce", ref).splitlines() if e.strip()}


def check_message(info: CommitInfo, cfg: Dict[str, Any], task: str = "") -> List[str]:
    problems = []
    commit_cfg = cfg["commit"]
    if not re.search(commit_cfg["subject_pattern"], info.subject):
        problems.append("its subject is not a conventional commit (`type(scope): summary`)")
    if task and commit_cfg.get("require_task_id") and task not in info.subject:
        problems.append(f"its subject does not name the task {task}")
    lines = [ln for ln in info.message.split("\n") if ln.strip()]
    trailer_lines = [ln for ln in lines if re.match(r"(?i)co-authored-by:", ln.strip())]
    pattern = commit_cfg.get("trailer_pattern") or (re.escape(commit_cfg["trailer"]) if commit_cfg.get("trailer") else "")
    if pattern:
        if len(trailer_lines) != 1:
            problems.append(f"it has {len(trailer_lines)} Co-Authored-By trailers, not exactly one")
        elif not re.fullmatch(pattern, trailer_lines[0].strip()):
            problems.append(f"its trailer `{trailer_lines[0].strip()}` is not the configured one")
        elif lines[-1].strip() != trailer_lines[0].strip():
            problems.append("its trailer is not the message's last line")
    for finding in errors(hygiene(info.message, cfg)):
        problems.append(f"its message holds {finding.rule}: {finding.excerpt}")
    return problems


def added_lines(git: Git, sha: str) -> str:
    out = git.run("show", "--format=", "--no-color", "--no-ext-diff", "-U0", sha).out
    return "\n".join(ln[1:] for ln in out.splitlines() if ln.startswith("+") and not ln.startswith("+++"))


def large_added_files(git: Git, sha: str, limit_kb: int) -> List[Tuple[str, int]]:
    out = git("diff-tree", "-r", "--root", "--no-commit-id", "--diff-filter=A", sha)
    big = []
    for line in out.splitlines():
        meta, _, path = line.partition("\t")
        parts = meta.split()
        if len(parts) >= 4 and set(parts[3]) != {"0"}:
            size = int(git("cat-file", "-s", parts[3]))
            if size > limit_kb * 1000:
                big.append((path, size))
    return big


def in_scope(path: str, scope: List[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, glob) for glob in scope)


@dataclass
class CommitReport:
    picks: List[CommitInfo]
    merges: List[str]
    problems: List[str]  # each blocks
    notes: List[str]

    def files(self) -> List[str]:
        return sorted({f for c in self.picks for f in c.files})


def check_commits(git: Git, cfg: Dict[str, Any], meta: Dict[str, Any], shas: List[str],
                  merges: List[str], identity_ref: str = "") -> CommitReport:
    """Check the commits a lane would land: messages and trailers, links and secrets in what they
    add, large added files, the protected paths, paths outside the brief's scope, and (against
    identity_ref's history) author and committer addresses new to the repository."""
    report = CommitReport([], merges, [], [])
    task = str(meta.get("task") or "")
    scope = meta.get("scope") or []
    known = known_identities(git, identity_ref) if identity_ref else set()
    allowed = {e.lower() for e in cfg["hygiene"].get("allowed_emails", [])}
    for sha in shas:
        info = commit_info(git, sha)
        report.picks.append(info)
        short = info.sha[:8]
        for problem in check_message(info, cfg, task):
            report.problems.append(f"{short} {info.subject!r}: {problem}")
        if identity_ref:
            for role, email in zip(("author", "committer"), info.emails):
                address = email.lower()
                if address in known or address in allowed or address.endswith("users.noreply.github.com"):
                    continue
                report.problems.append(f"{short}: its {role} address {email!r} is new to the repository "
                                       "(check the worker's git user.email)")
        touched = [f for f in info.files if protected(f)]
        if touched:
            report.problems.append(f"{short}: changes the collaboration's own files, which a lane may not: "
                                   + ", ".join(touched[:10]))
        for finding in errors(hygiene(added_lines(git, sha), cfg, kind="diff")):
            report.problems.append(f"{short}: adds {finding.rule}: {finding.excerpt}")
        for path, size in large_added_files(git, sha, int(cfg["commit"].get("max_added_file_kb", 500))):
            report.problems.append(f"{short}: adds {path}, {size // 1000} kB, over the "
                                   f"{cfg['commit']['max_added_file_kb']} kB limit")
        if scope:
            outside = [f for f in info.files if not in_scope(f, scope)]
            if outside:
                report.problems.append(f"{short}: changes paths outside the brief's scope: {', '.join(outside[:10])}")
    if not shas:
        report.problems.append("there are no commits to land")
    if merges:
        report.notes.append(f"{len(merges)} merge commit(s) on the lane, not picked: "
                            + ", ".join(m[:8] for m in merges))
    return report


def lane_commits(git: Git, base_ref: str, lane_ref: str, exclude_ref: str = "") -> Tuple[List[str], List[str]]:
    """The lane's non-merge commits not yet on base_ref (oldest first), and its merge commits.
    Commits already picked, even with a changed context, are recognised by subject and author
    date, which cherry-pick keeps. Commits on exclude_ref (the remote integration branch, which
    the lane may have merged while base_ref lags behind it) are never the lane's."""
    candidates = [s for s in git("rev-list", "--reverse", "--no-merges", "--cherry-pick", "--right-only",
                                 f"{base_ref}...{lane_ref}").splitlines() if s]
    merges = [s for s in git("rev-list", "--merges", f"{base_ref}..{lane_ref}").splitlines() if s]
    if exclude_ref:
        own = set(git("rev-list", f"{exclude_ref}..{lane_ref}").splitlines())
        candidates = [s for s in candidates if s in own]
        merges = [s for s in merges if s in own]
    if not candidates:
        return [], merges
    landed = landed_keys(git, base_ref, candidates)
    picks = [s for s in candidates if commit_key(git, s) not in landed]
    return picks, merges


def commit_key(git: Git, sha: str) -> Tuple[str, str]:
    date, subject = git("show", "-s", "--format=%ad%x00%s", "--date=raw", sha).split("\x00", 1)
    return date.strip(), subject


def landed_keys(git: Git, base_ref: str, shas: List[str]) -> Dict[Tuple[str, str], str]:
    """(author date, subject) of base_ref's commits since the oldest of shas was written."""
    oldest = min(int(commit_key(git, s)[0].split()[0]) for s in shas)
    out = git("log", "--no-merges", f"--since={oldest - 1}", "--format=%H%x00%ad%x00%s", "--date=raw", base_ref)
    keys = {}
    for line in out.splitlines():
        if line:
            sha, date, subject = line.split("\x00", 2)
            keys[(date.strip(), subject)] = sha
    return keys


# --------------------------------------------------------------------------------------------
# The stamped gate log


GATE_HEADER = "gh-collab gate:"


def worktree_tree(git: Git, scratch: Path) -> str:
    """The tree of the working tree as it stands, untracked files included, ignored ones not,
    built in a copy of the index so that the real one is untouched."""
    index = Path(git("rev-parse", "--git-path", "index"))
    if not index.is_absolute():
        index = Path(git.cwd or ".") / index
    temp = scratch / f"gate-index-{os.getpid()}"
    try:
        if index.exists():
            shutil.copyfile(index, temp)
        env = {"GIT_INDEX_FILE": str(temp)}
        git.run("add", "-A", env=env)
        return git("write-tree", env=env)
    finally:
        if temp.exists():
            temp.unlink()


def gate_run(git: Git, log: Path, command: Sequence[str], scratch: Path,
             popen: Callable[..., Any] = subprocess.run) -> int:
    """Run the gate from the repository root with its output in `log`, stamped with HEAD and the
    tree it ran on, and end the log with `gh-collab gate: exit <n>`. One string runs through the
    shell; several are run as they are."""
    root = git.root()
    head = git("rev-parse", "HEAD")
    tree = worktree_tree(git, scratch)
    log.parent.mkdir(parents=True, exist_ok=True)
    shell = len(command) == 1
    shown = command[0] if shell else " ".join(command)
    with open(log, "w", encoding="utf-8") as fh:
        fh.write(f"{GATE_HEADER} head={head} tree={tree} started={iso(now_utc())} command={shown}\n")
        fh.flush()
        proc = popen(command[0] if shell else list(command), shell=shell, cwd=str(root), stdout=fh,
                     stderr=subprocess.STDOUT)
        code = proc.returncode
    with open(log, "a", encoding="utf-8") as fh:
        fh.write(f"\n{GATE_HEADER} exit {code}\n")
    return code


def gate_check(git: Git, log: Path) -> List[str]:
    """Why the log does not show a passing gate on HEAD's tree; empty when it does."""
    if not log.is_file():
        return [f"no gate log at {log}: run the gate first"]
    lines = log.read_text(encoding="utf-8", errors="replace").splitlines()
    header = lines[0] if lines else ""
    tail = [ln for ln in lines if ln.strip()]
    problems = []
    m = re.match(re.escape(GATE_HEADER) + r" head=(\S+) tree=(\S+)", header)
    if not m:
        return [f"{log} is not a stamped gate log (its first line is not `{GATE_HEADER} head=…`)"]
    if not tail or tail[-1].strip() != f"{GATE_HEADER} exit 0":
        problems.append(f"the gate did not pass: its last line is `{tail[-1].strip() if tail else ''}`")
    if not git.is_clean():
        problems.append("the working tree has uncommitted changes: commit them, then re-run the gate")
    head_tree = git("rev-parse", "HEAD^{tree}")
    if m.group(2) != head_tree:
        problems.append("the gate ran on a different tree than HEAD's: re-run it on the committed work")
    return problems


def gate_command_of(log: Path) -> str:
    """The command a stamped gate log ran."""
    if not log.is_file():
        return ""
    with open(log, encoding="utf-8", errors="replace") as fh:
        header = fh.readline().rstrip("\n")
    return header.split(" command=", 1)[1] if " command=" in header else ""


def gate_is_configured(logged: str, configured: str) -> bool:
    """Whether the log ran the configured gate, perhaps behind environment settings or a capping
    wrapper (`VAR=1 systemd-run … -- timeout 9000 <gate>`): its command ends with the gate's."""
    logged, configured = " ".join(logged.split()), " ".join(configured.split())
    return bool(configured) and (logged == configured or logged.endswith(" " + configured))


def gate_tail(log: Path, count: int = 15) -> str:
    if not log.is_file():
        return ""
    lines = log.read_text(encoding="utf-8", errors="replace").splitlines()
    return "\n".join(lines[-count:])


# --------------------------------------------------------------------------------------------
# The coordination log: a discussion both sides post status to


DISCUSSION_QUERY = (
    "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name)"
    "{discussion(number:$number){id number title url updatedAt lastEditedAt author{login} editor{login}"
    " comments(last:100){nodes{id databaseId author{login} body createdAt url"
    " replies(first:50){nodes{id databaseId author{login} body createdAt url}}}}}}}")
COMMENT_MUTATION = ("mutation($id:ID!,$body:String!){addDiscussionComment(input:{discussionId:$id,body:$body})"
                    "{comment{url}}}")


def side_tag(cfg: Dict[str, Any], role: str) -> str:
    """The tag of a side, `orchestrator` or `worker`. It begins each side's status posts and the
    orchestrator's comments on issues (answers, closing comments): both sides may post as one
    account, and the worker may read only the comments that begin with the orchestrator's tag.
    The worker's own comments carry its markers instead."""
    return str(((cfg.get("coordination") or {}).get("tags") or {}).get(role) or f"[{role}]")


# A Markdown block that must start its line to render: a heading, a quote, a list item, a table
# row, a code fence or HTML (a marker among them).
BLOCK_START_RE = re.compile(r"(?:#{1,6}(?:\s|$)|>|[-*+](?:\s|$)|\d{1,9}[.)](?:\s|$)|\||```|~~~|<)")


def tagged(cfg: Dict[str, Any], role: str, text: str) -> str:
    """The text, stripped, beginning with its side's tag: inline before prose, on a line of its own
    before a Markdown block, so that the block still renders. A text that already begins with the
    tag is left as it is. Markers anywhere in the text are untouched."""
    tag = side_tag(cfg, role)
    text = text.strip()
    if text.startswith(tag):
        return text
    if not text:
        return tag
    return f"{tag}\n\n{text}" if BLOCK_START_RE.match(text) else f"{tag} {text}"


def discussion_number(cfg: Dict[str, Any]) -> int:
    value = cfg.get("coordination", {}).get("discussion") or 0
    if isinstance(value, int):
        return value
    m = re.search(r"(?:discussions/|#)?(\d+)\s*$", str(value))
    if not m:
        raise CollabError(f"coordination.discussion {value!r} is not a discussion number or URL")
    return int(m.group(1))


@dataclass
class Discussion:
    id: str
    number: int
    title: str
    url: str
    author: str
    editor: Optional[str]
    edited: Optional[datetime]
    comments: List[Comment]  # replies included, oldest first


def fetch_discussion(gh: Gh, cfg: Dict[str, Any]) -> Discussion:
    number = discussion_number(cfg)
    if not number:
        raise CollabError("no coordination log is configured (coordination.discussion)")
    owner, name = gh.repo.split("/", 1)
    data = gh.json("api", "graphql", "-f", f"query={DISCUSSION_QUERY}", "-f", f"owner={owner}",
                   "-f", f"name={name}", "-F", f"number={number}")
    node = (((data or {}).get("data") or {}).get("repository") or {}).get("discussion")
    if not node:
        raise CollabError(f"discussion #{number} not found in {gh.repo}")

    def comment(raw: Dict[str, Any]) -> Comment:
        return Comment(int(raw.get("databaseId") or 0), str(raw.get("id") or ""),
                       (raw.get("author") or {}).get("login") or "", raw.get("body") or "",
                       parse_time(raw.get("createdAt") or "1970-01-01T00:00:00Z"), raw.get("url") or "")

    comments = []
    for raw in (node.get("comments") or {}).get("nodes") or []:
        comments.append(comment(raw))
        comments.extend(comment(r) for r in (raw.get("replies") or {}).get("nodes") or [])
    edited = node.get("lastEditedAt")
    return Discussion(str(node["id"]), int(node["number"]), node.get("title") or "", node.get("url") or "",
                      (node.get("author") or {}).get("login") or "", (node.get("editor") or {}).get("login"),
                      parse_time(edited) if edited else None, sorted(comments, key=lambda c: (c.created, c.id)))


def post_discussion_comment(gh: Gh, discussion_id: str, body: str) -> str:
    payload = json.dumps({"query": COMMENT_MUTATION, "variables": {"id": discussion_id, "body": body}})
    data = json.loads(gh.call("api", "graphql", "--input", "-", stdin=payload).out or "{}")
    if data.get("errors"):
        raise CollabError(f"posting to the discussion failed: {data['errors'][0].get('message', data['errors'])}")
    return ((((data.get("data") or {}).get("addDiscussionComment") or {}).get("comment")) or {}).get("url", "")


# --------------------------------------------------------------------------------------------
# The optional Projects (v2) board


def token_scopes(gh: Gh) -> Optional[set]:
    """The classic token's OAuth scopes, or None when the token reports none (a fine-grained or
    app token, whose access shows only when it is used)."""
    res = gh.call("api", "--include", "user", check=False)
    if res.code != 0:
        return None
    for line in res.out.splitlines():
        if not line.strip():
            break  # the end of the headers
        if line.lower().startswith("x-oauth-scopes:"):
            return {s.strip() for s in line.split(":", 1)[1].split(",") if s.strip()}
    return None


SCOPE_HINT = "the token lacks the `project` scope; run `gh auth refresh -s project` on this machine"
BOARD_KEYS = ("queued", "claimed", "started", "done", "blocked", "landed")


class Board:
    """The board's project, its status field and that field's options, looked up once."""

    def __init__(self, cfg: Dict[str, Any], gh: Gh) -> None:
        self.cfg = cfg["project"]
        self.repo_owner = cfg["repo"].split("/", 1)[0] if cfg.get("repo") else ""
        self.gh = gh
        self._usable: Optional[Tuple[bool, str]] = None
        self._project: Optional[Dict[str, Any]] = None
        self._field: Optional[Tuple[str, Dict[str, str]]] = None

    @property
    def owner(self) -> str:
        return self.cfg.get("owner") or self.repo_owner

    def usable(self) -> Tuple[bool, str]:
        if self._usable is None:
            if not self.cfg.get("enabled"):
                self._usable = (False, "no board is configured (project.enabled is false)")
            elif not (self.cfg.get("number") or self.cfg.get("title")):
                self._usable = (False, "project.number or project.title must name the board")
            else:
                scopes = token_scopes(self.gh)
                if scopes is not None and "project" not in scopes:
                    self._usable = (False, SCOPE_HINT)
                else:
                    self._usable = (True, "")
        return self._usable

    def project(self, required: bool = True) -> Optional[Dict[str, Any]]:
        if self._project is None:
            data = self.gh.json("project", "list", "--owner", self.owner, "--format", "json", "--limit", "100") or {}
            for proj in data.get("projects") or []:
                if (self.cfg.get("number") and proj.get("number") == self.cfg["number"]) or (
                        not self.cfg.get("number") and proj.get("title") == self.cfg.get("title")):
                    self._project = proj
        if self._project is None and required:
            raise CollabError(f"board {self.cfg.get('number') or self.cfg.get('title')!r} not found for "
                              f"{self.owner}: run brief-issue's board-setup")
        return self._project

    def field(self) -> Tuple[str, Dict[str, str]]:
        if self._field is None:
            number = str(self.project()["number"])
            data = self.gh.json("project", "field-list", number, "--owner", self.owner, "--format", "json",
                                "--limit", "100") or {}
            for fld in data.get("fields") or []:
                if fld.get("name") == self.cfg["status_field"]:
                    options = {o.get("name", "").lower(): o.get("id", "") for o in fld.get("options") or []}
                    self._field = (fld.get("id", ""), options)
            if self._field is None:
                raise CollabError(f"the board has no {self.cfg['status_field']!r} field")
        return self._field

    def wanted_options(self) -> List[str]:
        wanted: List[str] = []
        for key in BOARD_KEYS:
            if self.cfg["statuses"][key] not in wanted:
                wanted.append(self.cfg["statuses"][key])
        return wanted

    def missing_options(self) -> List[str]:
        _, options = self.field()
        return [name for name in self.wanted_options() if name.lower() not in options]

    def items(self) -> Dict[str, Dict[str, Any]]:
        """The board's items by their issue URL."""
        number = str(self.project()["number"])
        data = self.gh.json("project", "item-list", number, "--owner", self.owner, "--format", "json",
                            "--limit", "1000") or {}
        return {(i.get("content") or {}).get("url", ""): i for i in data.get("items") or []}

    def set_status(self, issue_url: str, key: str) -> str:
        status = self.cfg["statuses"][key]
        field_id, options = self.field()
        option = options.get(status.lower())
        if not option:
            raise CollabError(f"the {self.cfg['status_field']!r} field has no {status!r} option: "
                              "add it in the board's settings")
        proj = self.project()
        item = self.gh.json("project", "item-add", str(proj["number"]), "--owner", self.owner, "--url", issue_url,
                            "--format", "json") or {}
        if not item.get("id"):
            raise CollabError("adding the issue to the board returned no item")
        self.gh.call("project", "item-edit", "--id", item["id"], "--project-id", proj["id"], "--field-id",
                     field_id, "--single-select-option-id", option)
        return status


def board_update(cfg: Dict[str, Any], gh: Gh, issue_url: str, key: str, board: Optional[Board] = None) -> str:
    """Move an issue on the board. Never fails the step it follows: a problem is reported instead."""
    board = board or Board(cfg, gh)
    ok, why = board.usable()
    if not ok:
        return f"board: skipped ({why})"
    try:
        return f"board: {board.set_status(issue_url, key)}"
    except CollabError as e:
        return f"board: not updated ({e})"


def board_key(issue: Issue, cfg: Dict[str, Any]) -> str:
    """The board status the labels and markers imply: started is a claim whose worker has begun."""
    labels = cfg["labels"]
    if issue.state == "CLOSED":
        return "landed"
    if issue.has(labels["blocked"]):
        return "blocked"
    if issue.has(labels["done"]):
        return "done"
    if issue.has(labels["claimed"]):
        claim = winning_claim(issue, cfg)
        if claim is not None:
            for comment in trusted_comments(issue, cfg):
                if comment.created >= claim.comment.created and any(k == "start" for k, _ in comment.markers()):
                    return "started"
        return "claimed"
    return "queued"


# --------------------------------------------------------------------------------------------
# Small shared CLI pieces


@dataclass
class Ctx:
    cfg: Dict[str, Any]
    gh: Gh
    git: Git
    now: Callable[[], datetime] = now_utc
    sleep: Callable[[float], None] = lambda s: __import__("time").sleep(s)


def make_ctx(resolve: bool = True) -> Ctx:
    git = Git()
    try:
        git.root()
    except CollabError:
        raise CollabError("run this from inside the repository's clone", EXIT_USAGE)
    cfg = load_config(git)
    gh = Gh(cfg.get("repo", ""))
    if resolve:
        cfg = resolve_repo(cfg, gh)
    return Ctx(cfg, gh, git)


def read_body(path: str) -> str:
    """A text to post, from a file or, for `-`, from standard input (a worktree-isolated agent
    can write no file outside its worktree, and a report has no place inside it)."""
    if not path:
        return ""
    if path == "-":
        return sys.stdin.read()
    try:
        return Path(path).read_text(encoding="utf-8")
    except OSError as e:
        raise CollabError(f"can't read {path}: {e}", EXIT_USAGE)


def main_guard(fn: Callable[[], int]) -> int:
    try:
        return fn()
    except CollabError as e:
        print(f"error: {e}", file=sys.stderr)
        return e.code
    except KeyboardInterrupt:
        return 130


def _cli(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("config", help="print the effective configuration")
    hyg = sub.add_parser("hygiene", help="check text bound for the public repository")
    hyg.add_argument("files", nargs="+")
    hyg.add_argument("--kind", choices=["brief", "comment", "diff"], default="comment")
    args = parser.parse_args(argv)

    def body() -> int:
        if args.cmd == "config":
            ctx = make_ctx(resolve=False)
            print(json.dumps(ctx.cfg, indent=2, sort_keys=True))
            return EXIT_OK
        cfg = load_config(Git())
        bad = 0
        for name in args.files:
            text = Path(name).read_text(encoding="utf-8")
            for finding in hygiene(text, cfg, args.kind):
                print(f"{name}: {finding}")
                bad += finding.level == "error"
        print("hygiene: clean" if not bad else f"hygiene: {bad} error(s)")
        return EXIT_FAIL if bad else EXIT_OK

    return main_guard(body)


if __name__ == "__main__":
    sys.exit(_cli())
