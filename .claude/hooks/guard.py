#!/usr/bin/env python3
"""security-guardrails: PreToolUse guard.

Runs before every Bash, Read, Edit, Write, MultiEdit, NotebookEdit, Grep and
Glob call, reads the Claude Code event (JSON on stdin) and answers one of:

  deny  - never legitimate: reading secrets, leaking tokens, destructive git
  ask   - sometimes legitimate: the user decides (even in auto mode)
  (nothing) - normal permission flow and the sandbox decide

What it checks
--------------
1. Secret paths, named by any tool or anywhere in a shell command.
2. Secrets that are not files: `gh auth token`, `git credential fill`,
   environment dumps, keyring tools, `$..._TOKEN` expansions.
3. `gh` used to leak data or take over the account (gists, SSH keys, ...).
4. Destructive git (force push, reset --hard, clean -f, ...), skipping hooks.
5. Edits that weaken the guardrails (settings.json, this script, the rules).
6. A first `git commit` in a repo without the gitleaks pre-commit hook.
7. A sandbox-exempt command (sandbox.excludedCommands: gh, git push...) chained
   with others, which would run sandboxed and fail as if signed out.

Limits
------
This is a heuristic over command text, not a sandbox. The Claude Code sandbox
(enabled in settings.json) is the layer that cannot be talked around; this
guard covers what runs outside it (gh, git push, the file tools) and catches
the obvious cases early with a clear reason.

Failure policy: any internal error blocks the call (exit 2). settings.json
also sets onFailure: block, so a crash or a timeout never lets a call through.
Every deny and ask is logged to .claude/hooks/guard.log (the hook writes a
.claude/hooks/.gitignore for it on first use).
"""
import fnmatch
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import time

# ---------------------------------------------------------------------------
# 1. Secret paths
# ---------------------------------------------------------------------------

# Sensitive wherever they appear as a path segment.
SENSITIVE_DIR_SEGMENTS = {".ssh", ".aws", ".gnupg", ".azure", ".kube", ".oci", ".password-store"}

# Sensitive only in the home directory: ./.codex and ./.gemini are legitimate
# repo-local project config folders.
HOME_ANCHORED_DIR_SEGMENTS = (".codex", ".gemini")

# Multi-segment suffixes (the parent folder itself is fine).
SENSITIVE_PATH_SUFFIXES = (
    ".config/gcloud", ".config/gh", ".config/anthropic", ".config/azure",
    ".docker/config.json", ".claude/.credentials.json", ".local/share/keyrings",
)

# Basename globs, sensitive wherever they live.
SENSITIVE_FILE_GLOBS = (
    ".env", ".env.*",
    "*.pem", "*.key", "*.p12", "*.pfx", "*.jks", "*.keystore",
    "id_rsa", "id_rsa.*", "id_dsa", "id_dsa.*", "id_ecdsa", "id_ecdsa.*",
    "id_ed25519", "id_ed25519.*",
    "*service-account*.json", "*credentials*.json",
    "secrets.*", "*.secret",
    "*.tfstate", "*.tfstate.*",
    ".netrc", ".git-credentials", ".npmrc", ".pypirc",
    ".pgpass", ".my.cnf", ".vault-token",
    ".claude.json",
    ".bash_history", ".zsh_history", ".python_history", ".psql_history",
)

PROC_ENVIRON_RE = re.compile(r"^/proc/[^/]+/environ$")

# Splits a token into the path-like pieces it may hide (quotes, =, parens...).
_PIECE_SPLIT_RE = re.compile(r"""[\s;|&()<>"'`,={}\[\]]+""")


def path_is_sensitive(path):
    """True if `path` names a protected credential / secret location."""
    p = (path or "").strip().strip("'\"").lstrip("@")  # curl -d @file, -F x=@file
    if not p:
        return False
    p = p.replace("${HOME}/", "~/").replace("$HOME/", "~/").rstrip("/")
    if PROC_ENVIRON_RE.match(p):
        return True
    low = p.lower()

    padded = "/" + low
    for suffix in SENSITIVE_PATH_SUFFIXES:
        if ("/" + suffix) in padded:
            return True

    home = os.path.expanduser("~").rstrip("/")
    for seg in HOME_ANCHORED_DIR_SEGMENTS:
        for anchor in ("~/" + seg, home + "/" + seg):
            if p == anchor or p.startswith(anchor + "/"):
                return True

    segments = [s for s in p.split("/") if s]
    for seg in segments:
        if seg in SENSITIVE_DIR_SEGMENTS:
            return True

    base = segments[-1] if segments else p
    for pattern in SENSITIVE_FILE_GLOBS:
        if fnmatch.fnmatch(base, pattern):
            return True
    return False


def first_sensitive(tokens, split_prose=False):
    """First token (or piece of a token) that names a secret path.

    A token with spaces in it was a quoted string: a commit message or an echo
    text such as "ignore .env files". It is checked as a whole path, and only
    split into words when `split_prose` is set (code passed to an interpreter,
    like python -c "open('/home/u/.ssh/id_rsa')")."""
    for tok in tokens:
        if path_is_sensitive(tok):
            return tok
        if split_prose or not re.search(r"\s", tok):
            for piece in _PIECE_SPLIT_RE.split(tok):
                if piece and piece != tok and path_is_sensitive(piece):
                    return piece
    return None


# ---------------------------------------------------------------------------
# Shell parsing: split a command line into simple commands (argv lists)
# ---------------------------------------------------------------------------

SHELL_KEYWORDS = {"if", "then", "else", "elif", "fi", "do", "done", "while", "until",
                  "for", "case", "esac", "in", "{", "}", "!", "time"}
SHELLS = {"sh", "bash", "zsh", "dash", "ksh", "fish"}
# Commands whose quoted arguments are code, not prose.
INTERPRETERS = SHELLS | {"python", "python3", "node", "perl", "ruby", "php", "deno", "bun",
                         "awk", "gawk", "osascript", "eval"}
ENV_ASSIGN_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")

# Wrappers that run another command, with the flags that take a value.
WRAPPERS = {
    "sudo": {"-u", "-g", "-C", "-D", "-h", "-p", "-r", "-t", "-U"},
    "doas": {"-u", "-C"},
    "command": set(), "builtin": set(), "exec": set(), "nohup": set(),
    "nice": {"-n"}, "ionice": {"-c", "-n", "-p"}, "chrt": set(),
    "stdbuf": {"-i", "-o", "-e"}, "setsid": set(), "unbuffer": set(),
    "timeout": {"-k", "-s"}, "xargs": {"-I", "-n", "-P", "-L", "-d", "-E", "-s", "-a"},
    "watch": {"-n", "-d"}, "strace": {"-e", "-o", "-p"}, "ltrace": {"-e", "-o", "-p"},
    "flock": {"-w", "-E"},
}
# Wrappers whose first positional argument is not the command (duration, lock file).
WRAPPER_SKIP_POSITIONAL = {"timeout", "flock"}


def _is_separator(tok):
    return bool(tok) and set(tok) <= set("();|&")


def _tokenize(text):
    prepared = (text.replace("\r", " ").replace("\n", " ; ")
                .replace("$(", " ( ").replace("`", " ( "))
    try:
        lex = shlex.shlex(prepared, posix=True, punctuation_chars=True)
        lex.whitespace_split = True
        return list(lex)
    except ValueError:
        # Unbalanced quotes: fall back to a naive split that still sees every word.
        return [t.strip("'\"") for t in re.findall(r"[^\s;|&()]+|[;|&()]+", prepared)]


def _strip_wrappers(argv):
    """Drop env assignments, shell keywords and wrapper commands.

    Returns the argv of the command that actually runs. A bare `env` (which
    dumps the environment) is returned as ["env"]."""
    argv = list(argv)
    while argv:
        head = argv[0]
        name = os.path.basename(head)
        if head in SHELL_KEYWORDS or ENV_ASSIGN_RE.match(head):
            argv.pop(0)
            continue
        if name == "env":
            rest = argv[1:]
            i = 0
            while i < len(rest):
                r = rest[i]
                if r in ("-u", "--unset", "-C", "--chdir", "-S", "--split-string"):
                    i += 2
                elif r.startswith("-") or ENV_ASSIGN_RE.match(r):
                    i += 1
                else:
                    break
            if i >= len(rest):
                return ["env"]
            argv = rest[i:]
            continue
        if name in WRAPPERS:
            value_flags = WRAPPERS[name]
            rest = argv[1:]
            i = 0
            while i < len(rest) and rest[i].startswith("-"):
                i += 2 if rest[i] in value_flags else 1
            if name in WRAPPER_SKIP_POSITIONAL and i < len(rest):
                i += 1
            argv = rest[i:]
            continue
        break
    return argv


def simple_commands(text, depth=0):
    """List of argv lists for every simple command in `text`, recursing into
    `sh -c '...'`, `eval ...`, subshells and command substitutions."""
    commands, current = [], []
    for tok in _tokenize(text) + [";"]:
        if _is_separator(tok):
            if current:
                commands.append(current)
            current = []
        else:
            current.append(tok)

    result = []
    for raw in commands:
        argv = _strip_wrappers(raw)
        if not argv:
            continue
        result.append(argv)
        if depth >= 3:
            continue
        name = os.path.basename(argv[0])
        if name in SHELLS:
            for i, a in enumerate(argv[1:-1], start=1):
                if a.startswith("-") and not a.startswith("--") and "c" in a:
                    result.extend(simple_commands(argv[i + 1], depth + 1))
                    break
        elif name == "eval" and len(argv) > 1:
            result.extend(simple_commands(" ".join(argv[1:]), depth + 1))
    return result


def short_cluster(tok):
    """Letters of a short-option cluster like -fdx, else ''."""
    if tok.startswith("-") and not tok.startswith("--") and len(tok) > 1 and tok[1:].isalpha():
        return tok[1:]
    return ""


# ---------------------------------------------------------------------------
# 2-4. Command rules
# ---------------------------------------------------------------------------

SECRET_VAR_RE = re.compile(r"\$\{?([A-Za-z_][A-Za-z0-9_]*)")
SECRET_NAME_RE = re.compile(
    r"(TOKEN|SECRET|PASSW(OR)?D|CREDENTIAL|API_?KEY|PRIVATE_?KEY|ACCESS_?KEY|(^|_)KEY$|(^|_)AUTH($|_))",
    re.IGNORECASE)
SECRET_NAME_ALLOW = {"SSH_AUTH_SOCK"}


def rule_secret_vars(command):
    for name in SECRET_VAR_RE.findall(command):
        if name.upper() not in SECRET_NAME_ALLOW and SECRET_NAME_RE.search(name):
            return ("deny", "expands the secret-looking variable $%s. Secret values must "
                            "never reach the transcript, a file or a request." % name)
    return None


def rule_env_and_keyrings(argv):
    name = os.path.basename(argv[0])
    args = argv[1:]
    if name == "printenv" or argv == ["env"]:
        return ("deny", "dumps the environment, which holds tokens and keys.")
    if name == "set" and not args:
        return ("deny", "dumps every shell variable, including secrets.")
    if name == "export" and (not args or args == ["-p"]):
        return ("deny", "dumps exported variables, including secrets.")
    if name in ("declare", "typeset") and args and all(a.startswith("-") for a in args) \
            and any(c in "".join(args) for c in "xp"):
        return ("deny", "dumps exported variables, including secrets.")
    if name in ("secret-tool", "kwallet-query"):
        return ("deny", "reads the system keyring, where gh and other tools keep tokens.")
    if name == "security" and args and (args[0].startswith("find-") or args[0] == "dump-keychain"):
        return ("deny", "reads the macOS keychain.")
    if name == "keyctl" and args and args[0] in ("print", "pipe", "read"):
        return ("deny", "reads the kernel keyring.")
    if name == "pass" and (not args or args[0] not in ("ls", "list", "find", "search", "grep", "init", "help", "version")):
        return ("deny", "reads the pass password store.")
    return None


GH_API_VALUE_FLAGS = {"-X", "--method", "-f", "--raw-field", "-F", "--field", "-H", "--header",
                      "--input", "-q", "--jq", "-t", "--template", "--hostname", "-p",
                      "--preview", "--cache"}
GH_API_FIELD_FLAGS = ("-f", "--raw-field", "-F", "--field", "--input")


def _gh_api(args):
    method, endpoint, has_fields = None, None, False
    i = 0
    while i < len(args):
        a = args[i]
        if a in ("-X", "--method"):
            method = args[i + 1] if i + 1 < len(args) else None
            i += 2
            continue
        if a.startswith("--method="):
            method = a.split("=", 1)[1]
        elif a.startswith("-X") and len(a) > 2:
            method = a[2:]
        elif a in GH_API_FIELD_FLAGS or any(a.startswith(f + "=") for f in GH_API_FIELD_FLAGS if f.startswith("--")) \
                or (a[:2] in ("-f", "-F") and len(a) > 2):
            has_fields = True
            if a in GH_API_FIELD_FLAGS:
                i += 2
                continue
        elif a in GH_API_VALUE_FLAGS:
            i += 2
            continue
        elif not a.startswith("-") and endpoint is None:
            endpoint = a
        i += 1
    effective = (method or ("POST" if has_fields else "GET")).upper()
    if (endpoint or "").strip("/") == "graphql":
        return None
    if effective != "GET":
        return ("ask", "`gh api` %s changes data on GitHub outside the normal issue/board "
                       "commands." % effective)
    return None


def rule_gh(argv):
    if os.path.basename(argv[0]) != "gh" or len(argv) < 2 or argv[1].startswith("-"):
        return None
    sub, rest = argv[1], argv[2:]
    subsub = rest[0] if rest and not rest[0].startswith("-") else ""
    if sub == "auth":
        if subsub == "token":
            return ("deny", "`gh auth token` prints your GitHub token.")
        if subsub == "status" and any(a in ("-t", "--show-token") or "t" in short_cluster(a) for a in rest[1:]):
            return ("deny", "`gh auth status --show-token` prints your GitHub token.")
        if subsub in ("login", "logout", "refresh", "setup-git", "switch"):
            return ("ask", "`gh auth %s` changes how gh is signed in." % subsub)
    if sub == "gist":
        return ("deny", "gists can publish any file, publicly, outside the repo.")
    if sub in ("ssh-key", "gpg-key") and subsub != "list":
        return ("deny", "`gh %s` changes the keys that can act as your account." % sub)
    if sub == "secret" and subsub != "list":
        return ("deny", "`gh secret` reads or writes repository secrets.")
    if sub == "repo":
        if subsub == "delete":
            return ("deny", "`gh repo delete` deletes a repository.")
        if subsub == "edit" and any(a == "--visibility" or a.startswith("--visibility=") for a in rest):
            return ("deny", "changing a repository's visibility can make private code public.")
    if sub in ("extension", "extensions", "ext") and subsub in ("install", "upgrade"):
        return ("deny", "gh extensions run third-party code with your GitHub token.")
    if sub == "api":
        return _gh_api(rest)
    if sub == "workflow" and subsub == "run":
        return ("ask", "`gh workflow run` starts CI with the repository's secrets.")
    return None


GIT_GLOBAL_VALUE_FLAGS = {"-C", "-c", "--git-dir", "--work-tree", "--namespace", "--config-env"}


def _git_parts(argv):
    """(subcommand, args, -C dir, list of -c settings)."""
    i, cdir, configs = 1, None, []
    while i < len(argv) and argv[i].startswith("-"):
        a = argv[i]
        if a in GIT_GLOBAL_VALUE_FLAGS:
            val = argv[i + 1] if i + 1 < len(argv) else ""
            if a == "-C":
                cdir = val
            elif a in ("-c", "--config-env"):
                configs.append(val)
            i += 2
        else:
            i += 1
    if i >= len(argv):
        return None, [], cdir, configs
    return argv[i], argv[i + 1:], cdir, configs


def rule_git(argv, cwd):
    if os.path.basename(argv[0]) != "git":
        return None
    sub, args, cdir, configs = _git_parts(argv)
    if any(c.lower().startswith("core.hookspath") for c in configs):
        return ("deny", "overriding core.hooksPath skips the repository's hooks (gitleaks).")
    if sub is None:
        return None
    clusters = "".join(short_cluster(a) for a in args)

    if sub == "credential" or sub.startswith("credential-"):
        return ("deny", "`git %s` reads or stores git credentials." % sub)
    if sub == "push":
        if any(a in ("--force", "--mirror", "--delete", "--prune", "--force-if-includes")
               or a.startswith("--force-with-lease") for a in args) or "f" in clusters or "d" in clusters:
            return ("deny", "force-pushing or deleting remote refs rewrites shared history.")
        if any(a.startswith("+") or (a.startswith(":") and len(a) > 1) for a in args if not a.startswith("-")):
            return ("deny", "a +refspec force-pushes and a :refspec deletes a remote branch.")
        if "--no-verify" in args:
            return ("deny", "`--no-verify` skips the repository's hooks.")
    if sub == "reset" and "--hard" in args:
        return ("deny", "`git reset --hard` throws away uncommitted work.")
    if sub == "clean" and ("--force" in args or "f" in clusters):
        return ("deny", "`git clean -f` deletes untracked files for good.")
    if sub == "branch" and ("D" in clusters or (("--delete" in args or "d" in clusters)
                                                 and ("--force" in args or "f" in clusters))):
        return ("deny", "`git branch -D` deletes a branch even if it was never merged.")
    if sub == "checkout" and ("." in args or "--force" in args or "f" in clusters):
        return ("deny", "this checkout throws away uncommitted changes.")
    if sub == "restore" and "." in args:
        staged_only = ("--staged" in args or "S" in clusters) and not ("--worktree" in args or "W" in clusters)
        if not staged_only:
            return ("deny", "`git restore .` throws away uncommitted changes.")
    if sub == "stash" and args[:1] == ["clear"]:
        return ("deny", "`git stash clear` deletes every stash for good.")
    if sub == "config" and any(a.lower() == "core.hookspath" for a in args) \
            and not any(a in ("--get", "--get-all", "--unset", "--unset-all", "--list", "-l") for a in args):
        return ("deny", "changing core.hooksPath disables the repository's hooks (gitleaks).")
    if sub == "commit":
        if "--no-verify" in args or "n" in clusters:
            return ("deny", "`--no-verify` skips the gitleaks pre-commit hook.")
        repo_dir = cdir if cdir and os.path.isabs(cdir) else os.path.join(cwd, cdir or "")
        return rule_gitleaks_hook(repo_dir)
    return None


GITLEAKS_SCRIPT = ".claude/skills/security-guardrails/scripts/install-gitleaks-hook.sh"


def rule_gitleaks_hook(repo_dir):
    """Deny the first commit in a repo that has no gitleaks pre-commit hook."""
    try:
        out = subprocess.run(["git", "-C", repo_dir, "rev-parse", "--git-path", "hooks"],
                             capture_output=True, text=True, timeout=5)
    except (OSError, subprocess.SubprocessError):
        return None
    if out.returncode != 0:
        return None  # not a git repository: git itself will refuse the commit
    hooks = out.stdout.strip()
    if not os.path.isabs(hooks):
        hooks = os.path.join(repo_dir, hooks)
    for name in ("pre-commit", "pre-commit-user"):
        try:
            with open(os.path.join(hooks, name), encoding="utf-8", errors="replace") as fh:
                if "gitleaks" in fh.read():
                    return None
        except OSError:
            pass
    if shutil.which("gitleaks") is None:
        log("note", "Bash", "gitleaks is not installed; commit allowed without a secret scan", repo_dir)
        return None
    return ("deny", "this repository has no gitleaks pre-commit hook yet. Run `bash %s` "
                    "(it installs the hook once), then commit again." % GITLEAKS_SCRIPT)


# ---------------------------------------------------------------------------
# 5. Guardrail self-protection
# ---------------------------------------------------------------------------

SETTINGS_SUFFIXES = ("/.claude/settings.json", "/.claude/settings.local.json")
GUARDED_SUFFIXES = ("/.claude/rules/security-guardrails.md",)
GUARDED_SEGMENTS = ("/.claude/hooks/", "/.claude/skills/security-guardrails/")

SHELL_WRITE_RE = re.compile(
    r"(>>?\s*\S*(\.claude/(settings|hooks|rules/security-guardrails)|security-guardrails))"
    r"|(\b(tee|rm|mv|cp|truncate|chmod|chown|ln)\b|\bsed\b[^;|&]*\s-i|\bperl\b[^;|&]*\s-i)"
    r"[^;|&]*(\.claude/(settings|hooks|rules/security-guardrails)|security-guardrails)")


def _abspath(path, cwd):
    p = os.path.expanduser(path or "")
    if not os.path.isabs(p):
        p = os.path.join(cwd, p)
    return os.path.normpath(p)


def security_view(d):
    d = d if isinstance(d, dict) else {}
    perms = d.get("permissions") if isinstance(d.get("permissions"), dict) else {}
    sandbox = d.get("sandbox") if isinstance(d.get("sandbox"), dict) else {}
    fs = sandbox.get("filesystem") if isinstance(sandbox.get("filesystem"), dict) else {}
    net = sandbox.get("network") if isinstance(sandbox.get("network"), dict) else {}
    guard, guard_strict = False, False
    hooks = d.get("hooks") if isinstance(d.get("hooks"), dict) else {}
    for entry in hooks.get("PreToolUse") or []:
        for h in (entry.get("hooks") or []) if isinstance(entry, dict) else []:
            if isinstance(h, dict) and "guard.py" in str(h.get("command", "")):
                guard = True
                guard_strict = guard_strict or h.get("onFailure") == "block"

    def strset(v):
        return set(x for x in v if isinstance(x, str)) if isinstance(v, list) else set()

    return {
        "deny": strset(perms.get("deny")),
        "guard": guard,
        "guard_strict": guard_strict,
        "disableAllHooks": d.get("disableAllHooks") is True,
        "bypass_default": perms.get("defaultMode") == "bypassPermissions",
        "bypass_disabled": perms.get("disableBypassPermissionsMode") == "disable",
        "sandbox_enabled": sandbox.get("enabled"),
        "fail_if_unavailable": sandbox.get("failIfUnavailable"),
        "allow_unsandboxed": sandbox.get("allowUnsandboxedCommands"),
        "excluded": strset(sandbox.get("excludedCommands")),
        "fs_disabled": fs.get("disabled") is True,
        "allow_read": strset(fs.get("allowRead")),
        "allow_write": strset(fs.get("allowWrite")),
        "deny_read": strset(fs.get("denyRead")),
        "deny_write": strset(fs.get("denyWrite")),
        "denied_domains": strset(net.get("deniedDomains")),
    }


def weakening_reasons(old, new):
    o, n = security_view(old), security_view(new)
    reasons = []
    removed = o["deny"] - n["deny"]
    if removed:
        reasons.append("removes deny rule(s): %s" % ", ".join(sorted(removed)[:5]))
    if o["guard"] and not n["guard"]:
        reasons.append("removes the guard.py hook")
    elif o["guard_strict"] and not n["guard_strict"]:
        reasons.append("drops onFailure: block from the guard hook")
    if n["disableAllHooks"] and not o["disableAllHooks"]:
        reasons.append("sets disableAllHooks")
    if n["bypass_default"] and not o["bypass_default"]:
        reasons.append("makes bypassPermissions the default mode")
    if o["bypass_disabled"] and not n["bypass_disabled"]:
        reasons.append("re-enables bypassPermissions mode")
    if n["sandbox_enabled"] is not True and (o["sandbox_enabled"] is True or n["sandbox_enabled"] is False):
        reasons.append("turns the sandbox off")
    if n["fail_if_unavailable"] is not True and o["fail_if_unavailable"] is True:
        reasons.append("lets Claude Code run without the sandbox when it can't start")
    if n["allow_unsandboxed"] is True and o["allow_unsandboxed"] is not True:
        reasons.append("lets commands retry outside the sandbox")
    if n["excluded"] - o["excluded"]:
        reasons.append("runs more commands outside the sandbox: %s" % ", ".join(sorted(n["excluded"] - o["excluded"])))
    if n["fs_disabled"] and not o["fs_disabled"]:
        reasons.append("turns off the sandbox's filesystem isolation")
    if (n["allow_read"] - o["allow_read"]) or (n["allow_write"] - o["allow_write"]):
        reasons.append("opens more paths in the sandbox")
    if (o["deny_read"] - n["deny_read"]) or (o["deny_write"] - n["deny_write"]):
        reasons.append("removes sandbox path restrictions")
    if o["denied_domains"] - n["denied_domains"]:
        reasons.append("removes denied network domains")
    return reasons


def _new_settings_text(tool, ti, old_text):
    if tool == "Write":
        return ti.get("content", "")
    if tool == "Edit":
        edits = [ti]
    elif tool == "MultiEdit":
        edits = ti.get("edits") or []
    else:
        return None
    text = old_text
    for e in edits:
        old_s, new_s = e.get("old_string", ""), e.get("new_string", "")
        if old_s == "":
            if text:
                return None
            text = new_s
            continue
        if old_s not in text:
            return None  # the edit tool will fail on its own
        text = text.replace(old_s, new_s) if e.get("replace_all") else text.replace(old_s, new_s, 1)
    return text


def rule_guarded_file(tool, ti, path):
    norm = "/" + path.lstrip("/")
    if norm.endswith(SETTINGS_SUFFIXES):
        try:
            with open(path, encoding="utf-8") as fh:
                old_text = fh.read()
        except OSError:
            old_text = ""
        new_text = _new_settings_text(tool, ti, old_text)
        if new_text is None:
            if tool == "NotebookEdit":
                return ("ask", "unusual edit to %s" % os.path.basename(path))
            return None
        try:
            new = json.loads(new_text) if new_text.strip() else {}
        except ValueError:
            return ("ask", "this edit leaves %s as invalid JSON, which would drop every "
                           "guardrail it holds." % os.path.basename(path))
        try:
            old = json.loads(old_text) if old_text.strip() else {}
        except ValueError:
            old = {}
        reasons = weakening_reasons(old, new)
        if reasons:
            return ("ask", "this edit weakens the security guardrails: %s." % "; ".join(reasons))
        return None
    if norm.endswith(GUARDED_SUFFIXES) or any(seg in norm for seg in GUARDED_SEGMENTS):
        return ("ask", "this edits the security guardrails themselves (%s)." % path)
    return None


# ---------------------------------------------------------------------------
# Decisions, logging, entry point
# ---------------------------------------------------------------------------

def project_dir(event):
    return os.environ.get("CLAUDE_PROJECT_DIR") or event.get("cwd") or os.getcwd()


_LOG_DIR = None


def log(decision, tool, reason, detail=""):
    try:
        if not _LOG_DIR:
            return
        folder = os.path.join(_LOG_DIR, ".claude", "hooks")
        path = os.path.join(folder, "guard.log")
        if not os.path.isdir(folder):
            return
        ignore = os.path.join(folder, ".gitignore")
        if not os.path.exists(ignore):  # keep the log out of git, wherever the template lands
            with open(ignore, "w", encoding="utf-8") as fh:
                fh.write("guard.log\n")
        line = json.dumps({
            "time": time.strftime("%Y-%m-%dT%H:%M:%S"),
            "decision": decision, "tool": tool, "reason": reason,
            "detail": (detail or "")[:200],
        })
        with open(path, "a", encoding="utf-8") as fh:
            fh.write(line + "\n")
    except Exception:
        pass


# ---------------------------------------------------------------------------
# 7. Sandbox-exempt commands chained with others
# ---------------------------------------------------------------------------

# Used when the project's settings.json can't be read.
DEFAULT_EXCLUDED_COMMANDS = (
    "gh *", "git push *", "git pull *", "git fetch *", "git clone *",
    "git ls-remote *", "git submodule *", "claude plugin *",
)


def excluded_commands(project):
    """The project's sandbox.excludedCommands patterns."""
    try:
        with open(os.path.join(project, ".claude", "settings.json")) as fh:
            patterns = (json.load(fh).get("sandbox") or {}).get("excludedCommands")
        if isinstance(patterns, list):
            return [p for p in patterns if isinstance(p, str) and p.strip()]
    except Exception:
        pass
    return list(DEFAULT_EXCLUDED_COMMANDS)


def _is_excluded(argv, patterns):
    line = " ".join(argv)
    for p in patterns:
        bare = p[:-2].rstrip() if p.endswith(" *") else p
        if line == bare or fnmatch.fnmatchcase(line, p):
            return True
    return False


def rule_exempt_in_compound(command, commands, patterns):
    """The sandbox exempts an excludedCommands entry only when it is the whole
    call. Chained (cd, &&, ;, |, $(...)) or prefixed (env VAR=, timeout), it
    runs sandboxed, without ~/.config/gh or ~/.ssh, and fails as if signed out."""
    hits = [argv for argv in commands if _is_excluded(argv, patterns)]
    if not hits:
        return None
    raw = _tokenize(command)
    alone = (len(commands) == 1 and not any(_is_separator(t) for t in raw)
             and "$(" not in command and "`" not in command
             and raw[:len(hits[0])] == hits[0])
    if alone:
        return None
    name = " ".join(hits[0][:2])
    return ("deny", "`%s` is exempt from the sandbox only when it is the whole command. "
            "Chained or prefixed (cd, &&, ;, |, $(...), env VAR=, timeout), it runs sandboxed "
            "without credentials and fails as 'not logged in' or 'permission denied'. Run it as "
            "its own Bash call with nothing before or after it (use flags like -R/--repo, "
            "--body-file or git -C instead of cd and substitutions)." % name, "retry")


def decide(decision, reason, tool, detail, retry=False):
    log(decision, tool, reason, detail)
    message = "Blocked by security-guardrails: " if decision == "deny" else "security-guardrails: "
    message += reason
    if decision == "deny" and not retry:
        message += " If this is genuinely needed, ask the user to do it themselves."
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": decision,
            "permissionDecisionReason": message,
        }
    }))
    sys.exit(0)


def pick(results):
    """deny wins over ask."""
    results = [r for r in results if r]
    for r in results:
        if r[0] == "deny":
            return r
    return results[0] if results else None


def check_bash(command, cwd):
    commands = simple_commands(command)
    for argv in commands:
        hit = first_sensitive(argv, os.path.basename(argv[0]) in INTERPRETERS)
        if hit:
            return ("deny", "this command touches a protected secret path ('%s')." % hit)
    results = [rule_secret_vars(command)]
    for argv in commands:
        results += [rule_env_and_keyrings(argv), rule_gh(argv), rule_git(argv, cwd)]
    if SHELL_WRITE_RE.search(command):
        results.append(("ask", "this shell command writes to the security guardrails' files."))
    # Last, so a security deny's reason wins over this one.
    results.append(rule_exempt_in_compound(command, commands, excluded_commands(_LOG_DIR or cwd)))
    return pick(results)


def main():
    global _LOG_DIR
    raw = sys.stdin.read()
    event = json.loads(raw)
    _LOG_DIR = project_dir(event)
    tool = event.get("tool_name", "")
    ti = event.get("tool_input") or {}
    cwd = event.get("cwd") or _LOG_DIR

    if tool == "Bash":
        command = ti.get("command", "") or ""
        result = check_bash(command, cwd)
        if result:
            decide(result[0], result[1], tool, command, retry=len(result) > 2)
    elif tool in ("Read", "Edit", "Write", "MultiEdit", "NotebookEdit"):
        fp = ti.get("file_path") or ti.get("notebook_path") or ""
        if path_is_sensitive(fp):
            decide("deny", "this %s call touches a protected secret path ('%s')." % (tool, fp), tool, fp)
        if tool != "Read":
            result = rule_guarded_file(tool, ti, _abspath(fp, cwd))
            if result:
                decide(result[0], result[1], tool, fp)
    elif tool in ("Grep", "Glob"):
        keys = ("path", "glob", "pattern") if tool == "Glob" else ("path", "glob")
        for key in keys:
            val = ti.get(key)
            if isinstance(val, str) and path_is_sensitive(val):
                decide("deny", "this %s call targets a protected secret path ('%s')." % (tool, val), tool, val)
    sys.exit(0)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as exc:  # fail closed
        sys.stderr.write("security-guardrails guard.py failed (%s: %s); the call is blocked "
                         "to stay safe. Check .claude/hooks/guard.py.\n" % (type(exc).__name__, exc))
        sys.exit(2)
