#!/usr/bin/env python3
"""A PreToolUse hook for the issue worker: it refuses tool calls that would push, publish, read
other accounts' comments, skip hooks, or change the collaboration's own files, except through
work_issue.py, which pushes only the lane branch.

Claude Code passes the tool call as JSON on stdin; exit 2 blocks the call and shows stderr to the
agent. Any failure here also exits 2, so the guard fails closed. It is defence in depth against
mistakes and text injected into the agent's context, not a sandbox: a determined shell can hide a
command from any pattern. The scripts' own checks, and the orchestrator's landing checks, are the
enforcement.

Refused in Bash (also inside `bash -c "…"`):
- `git push` (with git's global options), `--no-verify` anywhere, `git commit -n`, `core.hooksPath`,
  `git remote` changes, `git config` writes to remote, push, url, branch or credential keys, and
  `git credential`;
- every `gh` command that is not read-only, `gh auth token`, `gh api` with a method other than GET,
  fields or GraphQL, and reads of issue comments (`work_issue.py show` hides untrusted ones);
- each regex in the configuration's worker.deny_bash.
Refused for Edit, Write, MultiEdit and NotebookEdit: the protected paths (ghcollab.PROTECTED), the
collaboration's code, configuration and hooks.

Standard library only; Python 3.9+, run as `python3 -I`. Tests: test_work_issue.py beside this.
"""

from __future__ import annotations

import fnmatch
import json
import re
import sys
from pathlib import Path
from typing import List, Optional

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "gh-collab"))

try:
    import ghcollab
    PROTECTED = list(ghcollab.PROTECTED)
except Exception:  # noqa: BLE001  (the guard still works without the shared module)
    ghcollab = None
    PROTECTED = [".claude/gh-collab/*", ".claude/skills/*-issue*/*", ".claude/skills/post-status/*",
                 ".claude/agents/issue-worker.md", ".claude/settings.json", ".claude/settings.local.json"]

WORD = r"(?:^|(?<=[\s;&|(`/\"']))"  # the start of a command word, a path's last part, or a quoted command
GIT_VALUE_OPTS = r"--(?:git-dir|work-tree|namespace|super-prefix|config-env|exec-path)(?:=|\s+)\S+"
GIT = WORD + r"git(?:\s+(?:-C\s+\S+|-c\s*\S+|" + GIT_VALUE_OPTS + r"|--[\w-]+(?:=\S+)?|-[A-Za-z]+))*\s+"
GIT_RULES = [
    (re.compile(GIT + r"push\b"), "pushing is work_issue.py push's job: it pushes only the lane branch"),
    (re.compile(r"--no-verify\b"), "hooks are never skipped by hand"),
    (re.compile(r"core\.hookspath", re.I), "hooks are never redirected"),
    (re.compile(GIT + r"commit\b[^;&|\n]*\s-[A-Za-z]*n[A-Za-z]*(?=\s|$)"), "`git commit -n` skips the hooks"),
    (re.compile(GIT + r"remote\s+(?:add|set-url|rename|remove|rm|set-head|set-branches|prune)\b"),
     "the remotes are not changed"),
    (re.compile(GIT + r"credential\b"), "credentials are not read"),
]
GIT_CONFIG = re.compile(GIT + r"config\b([^;&|\n]*)")
GIT_CONFIG_KEY = re.compile(r"(?:^|\s)(?:remote|push|url|branch|credential)[.\w-]*", re.I)
GIT_CONFIG_READ = re.compile(r"\s(?:--get|--get-all|--get-regexp|--list|-l|--show-origin)\b")

GH = re.compile(WORD + r"gh\s+((?:[^;&|\n\"'])*)")
GH_READ = {
    "issue": {"list", "view", "status"},
    "pr": {"list", "view", "diff", "checks", "status"},
    "repo": {"view"},
    "label": {"list"},
    "run": {"list", "view", "watch"},
    "project": {"list", "view", "field-list", "item-list"},
    "auth": {"status"},
    "release": {"list", "view"},
    "search": None,  # any subcommand
    "status": None,
}
GH_API_WRITE = re.compile(r"(?:-X|--method)[\s=]*(?!GET\b)[A-Za-z]+|(?:^|\s)(?:-f|-F|--field|--raw-field|--input)"
                          r"(?=[\s=]|$)|(?:^|\s)-[fF]\S|(?:^|\s)graphql\b", re.I)
GH_COMMENTS = re.compile(r"--comments\b|--json\s+\S*comments|/comments\b|/timeline\b|discussions?\b", re.I)


def gh_problem(args: str) -> Optional[str]:
    words = args.split()
    if not words:
        return None
    top = words[0]
    if GH_COMMENTS.search(args):
        return "issue comments are read with work_issue.py show, which leaves out untrusted ones"
    if top == "api":
        if GH_API_WRITE.search(" " + args):
            return "`gh api` here is read-only: no methods but GET, no fields, no GraphQL"
        return None
    if top not in GH_READ:
        return f"`gh {top}` is not read-only"
    subs = GH_READ[top]
    sub = next((w for w in words[1:] if not w.startswith("-")), "")
    if subs is not None and sub not in subs:
        return f"`gh {top} {sub}` is not read-only: comments, labels and pushes go through work_issue.py"
    if "--show-token" in words:
        return "the token is not printed"
    return None


def deny_patterns() -> List[str]:
    """The configuration's extra patterns; a problem loading it leaves only the built-in rules."""
    try:
        return list(ghcollab.load_config(ghcollab.Git()).get("worker", {}).get("deny_bash", []))
    except Exception as e:  # noqa: BLE001  (the built-in rules still apply)
        print(f"gh-collab guard: the configuration's deny_bash was not loaded ({e})", file=sys.stderr)
        return []


def bash_problems(command: str, extra: List[str]) -> List[str]:
    found = []
    for rx, why in GIT_RULES:
        if rx.search(command):
            found.append(why)
    for m in GIT_CONFIG.finditer(command):
        if GIT_CONFIG_KEY.search(m.group(1)) and not GIT_CONFIG_READ.search(" " + m.group(1)):
            found.append("git configuration for remotes, pushes, branches and credentials is not changed")
    for m in GH.finditer(command):
        why = gh_problem(m.group(1))
        if why:
            found.append(why)
    for pattern in extra:
        if re.search(pattern, command):
            found.append(f"refused by the configuration (worker.deny_bash {pattern!r})")
    return found


def path_problem(path: str) -> Optional[str]:
    """A protected path, wherever the worktree is: matched from its last `.claude/` part."""
    norm = path.replace("\\", "/")
    at = norm.rfind(".claude/")
    if at < 0:
        return None
    rel = norm[at:]
    if any(fnmatch.fnmatchcase(rel, glob) for glob in PROTECTED):
        return f"{rel} belongs to the collaboration's own code, configuration or hooks, which a worker never edits"
    return None


def problems(event: dict, extra: Optional[List[str]] = None) -> List[str]:
    tool = event.get("tool_name")
    data = event.get("tool_input") or {}
    if tool == "Bash" or tool is None:
        return bash_problems(str(data.get("command") or ""), deny_patterns() if extra is None else extra)
    if tool in ("Edit", "Write", "MultiEdit", "NotebookEdit"):
        why = path_problem(str(data.get("file_path") or data.get("notebook_path") or ""))
        return [why] if why else []
    return []


def main(stdin: str, extra: Optional[List[str]] = None) -> int:
    try:
        found = problems(json.loads(stdin), extra)
    except Exception as e:  # noqa: BLE001  (fail closed)
        print(f"gh-collab guard failed, so the call is blocked: {e}", file=sys.stderr)
        return 2
    if found:
        print("gh-collab guard: blocked. " + "; ".join(dict.fromkeys(found)) + ". Use work_issue.py for "
              "claims, comments, labels and the lane push.", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.stdin.read()))
