"""Test support for the gh-collab scripts: a fake GitHub behind `gh`, and throwaway git
repositories with a local bare remote. Nothing here touches the network or a real repository.

The fake answers the `gh` calls the scripts make from an in-memory model of issues, labels,
milestones, a discussion and a Projects board; it records every call and fails the test on any
call it does not know. The git helpers isolate git from the user's configuration.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Sequence, Tuple

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import ghcollab as gc  # noqa: E402

OWNER = "owner"
REPO = "owner/repo"
TRAILER = "Co-Authored-By: Claude Test (1M context) <noreply@anthropic.com>"
T0 = datetime(2026, 1, 5, 12, 0, 0, tzinfo=timezone.utc)
WRITES = {("issue", "comment"), ("issue", "edit"), ("issue", "create"), ("issue", "close"), ("label", "create"),
          ("project", "item-add"), ("project", "item-edit"), ("project", "create"), ("project", "link")}


def iso(t: datetime) -> str:
    return gc.iso(t)


class FakeGitHub:
    """`gh` against a model of one repository. Call it as a runner: (argv, cwd, stdin, env)."""

    def __init__(self) -> None:
        self.issues: Dict[int, Dict[str, Any]] = {}
        self.labels = ["agent-task", "agent-claimed", "agent-done", "agent-blocked", "plan:R10", "area:sky",
                       "heavy", "bug"]
        self.milestones = [{"title": "RM5 Generated worlds (R09, R10)", "description": "Plans 09 and 10",
                            "open_issues": 1},
                           {"title": "Infrastructure", "description": "CI and tools", "open_issues": 0}]
        self.editors: Dict[int, Optional[str]] = {}
        self.actor = OWNER
        self.clock = T0
        self.next_id = 1000
        self.scopes: Optional[str] = "gist, project, read:org, repo"
        self.discussion: Dict[str, Any] = {"id": "D_1", "number": 7, "title": "Coordination log",
                                           "url": f"https://github.com/{REPO}/discussions/7", "author": OWNER,
                                           "comments": []}
        self.project = {"id": "PVT_1", "number": 2, "title": "Lanes", "url": "https://github.com/users/owner/projects/2"}
        self.options = {"Todo": "o1", "Claimed": "o2", "In progress": "o3", "Blocked": "o4", "Done": "o5"}
        self.items: Dict[str, Dict[str, Any]] = {}
        self.calls: List[Tuple[Tuple[str, ...], Optional[str]]] = []
        self.before_view: Optional[Callable[[int], None]] = None  # e.g. a rival's claim landing

    # -- the model --------------------------------------------------------------------------

    def tick(self) -> str:
        self.clock += timedelta(seconds=1)
        return iso(self.clock)

    def add_issue(self, number: int, title: str = "R10.T13 Something", body: str = "", author: str = OWNER,
                  labels: Sequence[str] = ("agent-task",), state: str = "OPEN", milestone: str = "") -> Dict[str, Any]:
        issue = {"number": number, "title": title, "body": body, "author": {"login": author},
                 "labels": [{"name": n} for n in labels], "state": state, "comments": [],
                 "milestone": {"title": milestone} if milestone else None, "createdAt": self.tick(),
                 "updatedAt": iso(self.clock), "closedAt": None,
                 "url": f"https://github.com/{REPO}/issues/{number}"}
        self.issues[number] = issue
        return issue

    def add_comment(self, number: int, body: str, author: Optional[str] = None, at: Optional[str] = None) -> int:
        self.next_id += 1
        cid = self.next_id
        self.issues[number]["comments"].append({
            "id": f"IC_{cid}", "author": {"login": author or self.actor}, "body": body, "createdAt": at or self.tick(),
            "url": f"https://github.com/{REPO}/issues/{number}#issuecomment-{cid}"})
        return cid

    def label_names(self, number: int) -> List[str]:
        return [lab["name"] for lab in self.issues[number]["labels"]]

    def writes(self) -> List[Tuple[str, ...]]:
        found = []
        for args, _ in self.calls:
            if tuple(args[:2]) in WRITES or (args[:1] == ("api",) and ("-X" in args or "--input" in args)):
                found.append(args)
        return found

    # -- the runner -------------------------------------------------------------------------

    def __call__(self, argv: Sequence[str], cwd: Optional[str] = None, stdin: Optional[str] = None,
                 env: Optional[Dict[str, str]] = None) -> gc.Result:
        if not argv or argv[0] != "gh":
            raise AssertionError(f"the fake gh runner got a non-gh command: {argv}")
        args = tuple(argv[1:])
        self.calls.append((args, stdin))
        out = self.dispatch(list(args), stdin)
        if isinstance(out, gc.Result):
            return out
        return gc.Result(0, out if isinstance(out, str) else json.dumps(out), "")

    @staticmethod
    def opt(args: List[str], name: str, default: Any = None) -> Any:
        return args[args.index(name) + 1] if name in args else default

    @staticmethod
    def opts(args: List[str], name: str) -> List[str]:
        return [args[i + 1] for i, a in enumerate(args) if a == name]

    def dispatch(self, a: List[str], stdin: Optional[str]) -> Any:  # noqa: C901  (one branch per command)
        if a[:2] == ["issue", "view"]:
            number = int(a[2])
            if self.before_view:
                self.before_view(number)
            if number not in self.issues:
                return gc.Result(1, "", "issue not found")
            return self.issues[number]
        if a[:2] == ["issue", "list"]:
            state = self.opt(a, "--state", "open").upper()
            label = self.opt(a, "--label")
            found = [i for i in self.issues.values() if i["state"] == state
                     and (label is None or label in [lab["name"] for lab in i["labels"]])]
            search = self.opt(a, "--search", "")
            m = re.match(r"closed:>=(\S+)", search)
            if m:
                found = [i for i in found if (i["closedAt"] or "") >= m.group(1)]
            return found
        if a[:2] == ["issue", "comment"]:
            self.add_comment(int(a[2]), stdin or "")
            return ""
        if a[:2] == ["issue", "edit"]:
            number = int(a[2])
            names = self.label_names(number)
            for lab in self.opts(a, "--add-label"):
                if lab not in self.labels:
                    return gc.Result(1, "", f"'{lab}' not found")
                if lab not in names:
                    names.append(lab)
            names = [n for n in names if n not in self.opts(a, "--remove-label")]
            self.issues[number]["labels"] = [{"name": n} for n in names]
            return ""
        if a[:2] == ["issue", "create"]:
            number = max(list(self.issues) + [0]) + 1
            labels = [lab for lab in (self.opt(a, "--label") or "").split(",") if lab]
            issue = self.add_issue(number, self.opt(a, "--title"), stdin or "", self.actor, labels,
                                   milestone=self.opt(a, "--milestone", ""))
            return issue["url"] + "\n"
        if a[:2] == ["issue", "close"]:
            number = int(a[2])
            self.add_comment(number, self.opt(a, "--comment", ""))
            self.issues[number]["state"] = "CLOSED"
            self.issues[number]["closedAt"] = iso(self.clock)
            return ""
        if a[:2] == ["label", "list"]:
            return [{"name": n} for n in self.labels]
        if a[:2] == ["label", "create"]:
            self.labels.append(a[2])
            return ""
        if a[:2] == ["repo", "view"]:
            return {"nameWithOwner": REPO, "owner": {"login": OWNER}, "defaultBranchRef": {"name": "trunk"}}
        if a[:1] == ["api"]:
            return self.api(a[1:], stdin)
        if a[:1] == ["project"]:
            return self.project_cmd(a[1:])
        raise AssertionError(f"unexpected gh call: {a}")

    def api(self, a: List[str], stdin: Optional[str]) -> Any:
        if a[:2] == ["--include", "user"]:
            head = "HTTP/2.0 200 OK\nContent-Type: application/json\n"
            if self.scopes is not None:
                head += f"X-Oauth-Scopes: {self.scopes}\n"
            return head + "\n" + json.dumps({"login": OWNER})
        if a[:2] == ["-X", "DELETE"]:
            cid = a[2].rsplit("/", 1)[1]
            for issue in self.issues.values():
                issue["comments"] = [c for c in issue["comments"] if not c["url"].endswith(f"-{cid}")]
            return ""
        if a and a[0].startswith(f"repos/{REPO}/milestones"):
            return self.milestones
        if a[:1] == ["graphql"]:
            if "--input" in a:
                payload = json.loads(stdin or "{}")
                assert "addDiscussionComment" in payload["query"], payload["query"]
                assert payload["variables"]["id"] == self.discussion["id"]
                self.next_id += 1
                url = f"{self.discussion['url']}#discussioncomment-{self.next_id}"
                self.discussion["comments"].append({"id": f"DC_{self.next_id}", "databaseId": self.next_id,
                                                    "author": {"login": self.actor},
                                                    "body": payload["variables"]["body"], "createdAt": self.tick(),
                                                    "url": url, "replies": {"nodes": []}})
                return {"data": {"addDiscussionComment": {"comment": {"url": url}}}}
            query = next(x for x in a if x.startswith("query="))
            number = int(next(x for x in a if x.startswith("number=")).split("=", 1)[1])
            if "discussion(" in query:
                d = self.discussion
                return {"data": {"repository": {"discussion": {
                    "id": d["id"], "number": d["number"], "title": d["title"], "url": d["url"],
                    "updatedAt": iso(T0), "lastEditedAt": None, "author": {"login": d["author"]}, "editor": None,
                    "comments": {"nodes": d["comments"]}}}}}
            editor = self.editors.get(number)  # None: never edited; "": edited by a deleted account
            return {"data": {"repository": {"issue": {"editor": {"login": editor} if editor else None,
                                                      "lastEditedAt": iso(T0) if editor is not None else None}}}}
        raise AssertionError(f"unexpected gh api call: {a}")

    def project_cmd(self, a: List[str]) -> Any:
        if self.scopes is not None and "project" not in self.scopes:
            return gc.Result(1, "", "your token has not been granted the required scopes")
        if a[0] == "list":
            return {"projects": [self.project], "totalCount": 1}
        if a[0] == "field-list":
            return {"fields": [{"id": "F_title", "name": "Title", "type": "ProjectV2Field"},
                               {"id": "F_status", "name": "Status", "type": "ProjectV2SingleSelectField",
                                "options": [{"id": i, "name": n} for n, i in self.options.items()]}]}
        if a[0] == "item-add":
            url = self.opt(a, "--url")
            item = self.items.setdefault(url, {"id": f"PVTI_{len(self.items) + 1}", "content": {"url": url}})
            return {"id": item["id"], "type": "Issue", "url": url}
        if a[0] == "item-edit":
            assert self.opt(a, "--project-id") == self.project["id"] and self.opt(a, "--field-id") == "F_status"
            name = {i: n for n, i in self.options.items()}[self.opt(a, "--single-select-option-id")]
            for item in self.items.values():
                if item["id"] == self.opt(a, "--id"):
                    item["status"] = name
            return ""
        if a[0] == "item-list":
            return {"items": list(self.items.values()), "totalCount": len(self.items)}
        if a[0] == "create":
            self.project = {"id": "PVT_new", "number": 3, "title": self.opt(a, "--title"), "url": "u"}
            return self.project
        if a[0] == "link":
            return ""
        raise AssertionError(f"unexpected gh project call: {a}")

    def status_of(self, number: int) -> Optional[str]:
        return (self.items.get(self.issues[number]["url"]) or {}).get("status")


# -- configuration and context ---------------------------------------------------------------


def make_config(scratch: Path, **over: Any) -> Dict[str, Any]:
    cfg = gc.deep_merge(gc.DEFAULTS, {
        "repo": REPO, "base_branch": "trunk", "allowed_authors": [OWNER],
        "task_id_pattern": r"(?P<plan>[PR]\d{2})\.T\d+(?:\.[a-z]+)?",
        "commit": {"trailer": TRAILER, "trailer_pattern": r"Co-Authored-By: Claude [^<>]+ <noreply@anthropic\.com>"},
        "worker": {"id": "w-test", "claim_settle_seconds": 0},
        "coordination": {"discussion": 7, "tags": {"orchestrator": "[desk]", "worker": "[lap]"}},
        "project": {"enabled": True, "number": 2, "owner": OWNER},
        "scratch_dir": str(scratch),
    })
    cfg = gc.deep_merge(cfg, over)
    gc.validate_config(cfg)
    return cfg


def make_ctx(fake: FakeGitHub, cfg: Dict[str, Any], cwd: Path, now: datetime = T0) -> gc.Ctx:
    return gc.Ctx(cfg, gc.Gh(cfg["repo"], runner=fake), gc.Git(str(cwd)), now=lambda: now, sleep=lambda s: None)


# -- git --------------------------------------------------------------------------------------


def scratch_root() -> Path:
    """Scratch for tests: $GH_COLLAB_TEST_TMP, else <this clone's git common dir>/gh-collab/test-tmp.
    Never the system's temporary directory, which may be held in memory."""
    if os.environ.get("GH_COLLAB_TEST_TMP"):
        root = Path(os.environ["GH_COLLAB_TEST_TMP"])
    else:
        out = subprocess.run(["git", "rev-parse", "--git-common-dir"], cwd=str(Path(__file__).parent),
                             capture_output=True, text=True, check=True).stdout.strip()
        common = Path(out) if Path(out).is_absolute() else (Path(__file__).parent / out).resolve()
        root = common / "gh-collab" / "test-tmp"
    root.mkdir(parents=True, exist_ok=True)
    return root


class Sandbox:
    """A temporary directory with git isolated from the user's configuration, and `gh` forbidden."""

    def __enter__(self) -> "Sandbox":
        self.dir = Path(tempfile.mkdtemp(prefix="t-", dir=str(scratch_root())))
        gitconfig = self.dir / "gitconfig"
        gitconfig.write_text("[init]\n\tdefaultBranch = trunk\n[advice]\n\tdetachedHead = false\n", encoding="utf-8")
        self.saved = dict(os.environ)
        os.environ.update({
            "GIT_CONFIG_GLOBAL": str(gitconfig), "GIT_CONFIG_NOSYSTEM": "1", "GIT_TERMINAL_PROMPT": "0",
            "GIT_AUTHOR_NAME": "Test", "GIT_AUTHOR_EMAIL": "test@users.noreply.github.com",
            "GIT_COMMITTER_NAME": "Test", "GIT_COMMITTER_EMAIL": "test@users.noreply.github.com",
            "GH_COLLAB_FORBID_GH": "1",
        })
        os.environ.pop("GH_COLLAB_CONFIG", None)
        return self

    def __exit__(self, *exc: Any) -> None:
        os.environ.clear()
        os.environ.update(self.saved)
        shutil.rmtree(self.dir, ignore_errors=True)


def git(cwd: Path, *args: str) -> str:
    proc = subprocess.run(["git", *args], cwd=str(cwd), capture_output=True, text=True)
    if proc.returncode != 0:
        raise AssertionError(f"git {' '.join(args)} failed: {proc.stderr}")
    return proc.stdout.strip()


def commit(cwd: Path, files: Dict[str, str], message: str, date: Optional[str] = None) -> str:
    for name, text in files.items():
        path = cwd / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        git(cwd, "add", name)
    env_date = {"GIT_AUTHOR_DATE": date, "GIT_COMMITTER_DATE": date} if date else {}
    saved = dict(os.environ)
    os.environ.update(env_date)
    try:
        git(cwd, "commit", "-q", "-m", message)
    finally:
        os.environ.clear()
        os.environ.update(saved)
    return git(cwd, "rev-parse", "HEAD")


def make_repos(root: Path) -> Tuple[Path, Path]:
    """A bare `remote.git` and a clone `main` on `trunk`, with one pushed commit."""
    remote = root / "remote.git"
    git(root, "init", "-q", "--bare", "-b", "trunk", str(remote))
    main = root / "main"
    git(root, "clone", "-q", str(remote), str(main))
    commit(main, {"README.md": "hello\n", "docs/plan.md": "plan\n"}, "docs: start")
    git(main, "push", "-q", "origin", "trunk")
    return remote, main


def msg(subject: str, body: str = "", trailer: str = TRAILER) -> str:
    return subject + ("\n\n" + body if body else "") + ("\n\n" + trailer if trailer else "")
