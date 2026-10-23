"""Platform-neutral checks behind the nuff-hooks plugins.

This file ships unchanged in plugins/nuff-hooks-cc/hooks and plugins/nuff-hooks-copilot/hooks.
Every copy must stay byte-identical with plugins/nuff-hooks/core (tests/test_sync.py checks it).
Terms (Workspace, Read Zone, Write Zone, Trash, Removal, Rewrite) are defined in
plugins/nuff-hooks/CONTEXT.md.

Only the Python standard library is used: the hooks start on every tool call.
"""

from __future__ import annotations

import glob
import os
import re
import shlex
import shutil
import tempfile
from dataclasses import dataclass, field

READ_ROOTS = ("D:\\",)
TRASH = "D:\\.slop-trash"
AGENT_DIRS = (".claude", ".claude.json", ".copilot")  # relative to the user's home

PREFIX = "nuff-hooks: "
READ_ZONE_TEXT = "the Read Zone is D:\\, the agent's home dirs and the temp dir"
WRITE_ZONE_TEXT = "the Write Zone is the Workspace, D:\\.slop-trash, the agent's home dirs and the temp dir"
REMOVAL_HINT = (
    "removals are moved to D:\\.slop-trash, but only a plain, standalone removal can be rewritten. "
    "Send it as its own command with literal paths or wildcards, "
    "e.g. `rm -rf <paths>` (bash) or `Remove-Item -Recurse -Force <paths>` (PowerShell)"
)

OUTSIDE = "<outside>"  # resolved path for UNC, device and unknown absolute paths


@dataclass
class Context:
    workspace: str
    cwd: str
    session_id: str
    env: dict = field(default_factory=lambda: dict(os.environ))


@dataclass
class Decision:
    kind: str  # "allow" | "deny" | "rewrite"
    reason: str = ""
    command: str = ""


ALLOW = Decision("allow")


class _Deny(Exception):
    """Stops a check at the first part that is not allowed."""


def _deny(reason: str) -> Decision:
    return Decision("deny", PREFIX + reason)


# --- paths and zones -------------------------------------------------------------------

_VAR = re.compile(r"\$env:(\w+)|\$\{(?:env:)?(\w+)\}|\$(\w+)|%(\w+)%", re.IGNORECASE)
_NULL_DEVICES = {"/dev/null", "nul", "nul:", "$null"}
_PIPELINE_VARS = re.compile(r"^\$(_|psitem|true|false|null)\b", re.IGNORECASE)


def _env(ctx: Context, name: str) -> str | None:
    upper = name.upper()
    if upper in ("HOME", "USERPROFILE"):
        return _home(ctx)
    if upper == "PWD":
        return ctx.cwd
    for key, value in ctx.env.items():
        if key.upper() == upper:
            return value
    return None


def _home(ctx: Context) -> str:
    for key, value in ctx.env.items():
        if key.upper() == "USERPROFILE":
            return value
    return os.path.expanduser("~")


def _absolute(path: str, cwd: str, shell: str) -> str | None:
    """Normalised absolute Windows path, OUTSIDE, or None for a null device."""
    if path.lower() in _NULL_DEVICES:
        return None
    if path.startswith(("\\\\", "//")):
        drive = re.match(r"^//([A-Za-z])(?:/|$)(.*)", path) if shell == "bash" else None
        if not drive:
            return OUTSIDE
        path = drive.group(1) + ":\\" + drive.group(2)
    elif shell == "bash" and path.startswith("/"):
        drive = re.match(r"^/(?:mnt/)?([A-Za-z])(?:/|$)(.*)", path)
        if drive:
            path = drive.group(1) + ":\\" + drive.group(2)
        elif path == "/tmp" or path.startswith("/tmp/"):
            path = tempfile.gettempdir() + path[4:]
        else:
            return OUTSIDE  # Git Bash's own tree (/usr, /etc, ...) lives on C:
    if not re.match(r"^[A-Za-z]:[\\/]", path):
        path = os.path.join(_absolute(cwd, "C:\\", "bash") or cwd, path)
    return os.path.normpath(path)


def resolve(token: str, ctx: Context, cwd: str, shell: str) -> str | None:
    """Resolve a path token; raises _Deny when it hides a path the hook can't know."""
    if token.lower() in _NULL_DEVICES:
        return None
    if "$(" in token or "`" in token:
        raise _Deny(f"cannot check '{token}': it uses command substitution")

    def expand(match: re.Match) -> str:
        name = next(group for group in match.groups() if group)
        value = _env(ctx, name)
        if value is None:
            raise _Deny(f"cannot check '{token}': variable '{match.group(0)}' is unknown to the hook")
        return value

    path = _VAR.sub(expand, token)
    if path == "~" or path.startswith(("~/", "~\\")):
        path = _home(ctx) + path[1:]
    return _absolute(path, cwd, shell)


def _under(path: str, root: str) -> bool:
    if path == OUTSIDE:
        return False
    path = os.path.normcase(path)
    root = os.path.normcase(_absolute(root, "C:\\", "bash") or root)
    if root.endswith(os.sep):
        return path.startswith(root)
    return path == root or path.startswith(root + os.sep)


def _agent_and_temp(ctx: Context) -> list[str]:
    home = _home(ctx)
    dirs = [os.path.join(home, name) for name in AGENT_DIRS]
    dirs += [d for d in (_env(ctx, "TEMP"), _env(ctx, "TMP"), tempfile.gettempdir()) if d]
    return dirs


def in_read_zone(path: str, ctx: Context) -> bool:
    return any(_under(path, root) for root in (*READ_ROOTS, *_agent_and_temp(ctx)))


def in_write_zone(path: str, ctx: Context) -> bool:
    return any(_under(path, root) for root in (ctx.workspace, TRASH, *_agent_and_temp(ctx)))


def in_trash(path: str) -> bool:
    return _under(path, TRASH)


def _require_read(token: str, ctx: Context, cwd: str, shell: str) -> None:
    path = resolve(token, ctx, cwd, shell)
    if path is not None and not in_read_zone(path, ctx):
        raise _Deny(f"reading '{token}' is not allowed: {READ_ZONE_TEXT}")


def _require_write(token: str, ctx: Context, cwd: str, shell: str) -> None:
    path = resolve(token, ctx, cwd, shell)
    if path is not None and not in_write_zone(path, ctx):
        raise _Deny(f"writing '{token}' is not allowed: {WRITE_ZONE_TEXT}")


# --- file tools ------------------------------------------------------------------------


def _guard(check) -> Decision:
    try:
        check()
    except _Deny as denied:
        return _deny(str(denied))
    return ALLOW


def check_read(path: str, ctx: Context) -> Decision:
    return _guard(lambda: _require_read(path or ".", ctx, ctx.cwd, "file"))


def check_write(path: str, ctx: Context) -> Decision:
    return _guard(lambda: _require_write(path or ".", ctx, ctx.cwd, "file"))


def check_search(path: str | None, pattern: str | None, ctx: Context) -> Decision:
    def check() -> None:
        _require_read(path or ".", ctx, ctx.cwd, "file")
        if pattern and re.match(r"^([A-Za-z]:|[\\/])", pattern):
            base = re.split(r"[*?\[{]", pattern, maxsplit=1)[0]
            _require_read(base or pattern, ctx, ctx.cwd, "file")

    return _guard(check)


_PATCH_FILE = re.compile(r"^\*\*\* (Add File|Update File|Delete File|Move to): (.+?)\s*$", re.MULTILINE)


def check_patch(patch: str, ctx: Context) -> Decision:
    """apply_patch: every touched file must be in the Write Zone; deletes are Removals."""

    def check() -> None:
        for action, path in _PATCH_FILE.findall(patch or ""):
            if action == "Delete File":
                raise _Deny(f"apply_patch deletes are not allowed; {REMOVAL_HINT}")
            _require_write(path, ctx, ctx.cwd, "file")

    return _guard(check)


# --- shell tokenizing ------------------------------------------------------------------


@dataclass
class Segment:
    argv: list[str] = field(default_factory=list)
    redirects: list[tuple[str, str]] = field(default_factory=list)  # (operator, target)


_SEPARATORS = {";", "&&", "||", "|", "|&", "&", "\n", "(", ")", "{", "}", ";;"}
_WRITE_REDIRECTS = {">", ">>", ">|", "&>", "&>>"}
_READ_REDIRECTS = {"<"}
_SKIP_REDIRECTS = {">&", "<&", "<<", "<<-", "<<<"}
_BASH_OPS = sorted(_SEPARATORS | _WRITE_REDIRECTS | _READ_REDIRECTS | _SKIP_REDIRECTS, key=len, reverse=True)
_HEREDOC = re.compile(r"<<-?\s*(['\"]?)(\w+)\1")


def _split_ops(token: str) -> list[str]:
    ops = []
    while token:
        op = next((op for op in _BASH_OPS if token.startswith(op)), token[0])
        ops.append(op)
        token = token[len(op):]
    return ops


def _strip_heredocs(command: str) -> str:
    kept, pending = [], []
    for line in command.split("\n"):
        if pending:
            if line.strip() == pending[0]:
                pending.pop(0)
            continue
        kept.append(line)
        pending.extend(match.group(2) for match in _HEREDOC.finditer(line))
    return "\n".join(kept)


def _segments(tokens: list[tuple[str, bool]]) -> list[Segment]:
    """Group (text, is_operator) tokens into segments."""
    segments, current = [], Segment()
    i = 0
    while i < len(tokens):
        text, is_op = tokens[i]
        i += 1
        if not is_op:
            current.argv.append(text)
            continue
        if text in _SEPARATORS:
            if current.argv or current.redirects:
                segments.append(current)
            current = Segment()
            continue
        if text == ">&dup":  # `2>&1`, already complete
            continue
        target = tokens[i][0] if i < len(tokens) and not tokens[i][1] else ""
        if target:
            i += 1
        if current.argv and re.fullmatch(r"\d", current.argv[-1]):
            current.argv.pop()  # the file descriptor in `2>file`
        if text in _SKIP_REDIRECTS or re.fullmatch(r"\d+|-", target):
            continue
        current.redirects.append((">" if text in _WRITE_REDIRECTS else "<", target))
    if current.argv or current.redirects:
        segments.append(current)
    return segments


def _tokenize_bash(command: str) -> list[tuple[str, bool]]:
    # Backticks run their content just like $( ); rewriting them keeps it checked in place.
    command = re.sub(r"`([^`]*)`", r"$(\1)", _strip_heredocs(command))
    lexer = shlex.shlex(command, posix=True, punctuation_chars="();<>|&\n")
    lexer.whitespace = " \t\r"
    lexer.whitespace_split = True
    tokens: list[tuple[str, bool]] = []
    for token in lexer:
        if token and all(char in "();<>|&\n" for char in token):
            ops = _split_ops(token)
            if ops[0] == "(" and tokens and not tokens[-1][1] and tokens[-1][0].endswith("$"):
                tokens[-1] = (tokens[-1][0] + "(", False)  # keep `$(` visible to resolve()
            tokens.extend((op, True) for op in ops)
        else:
            tokens.append((token, False))
    return tokens


def _tokenize_powershell(command: str) -> list[tuple[str, bool]]:
    tokens, word, i, n = [], [], 0, len(command)
    has_word = False

    def flush() -> None:
        nonlocal word, has_word
        if has_word:
            tokens.append(("".join(word), False))
        word, has_word = [], False

    while i < n:
        char = command[i]
        if char in " \t\r,":
            flush()
            i += 1
        elif char == "#" and not has_word:
            while i < n and command[i] != "\n":
                i += 1
        elif char == "`":
            if i + 1 < n and command[i + 1] != "\n":
                word.append(command[i + 1])
                has_word = True
            i += 2
        elif char == "'" or (char == "@" and command[i + 1 : i + 2] in ("'", '"') and command[i + 2 : i + 3] in ("\n", "\r")):
            here = char == "@"
            quote = command[i + 1] if here else "'"
            end = ("\n" + quote + "@") if here else "'"
            i += 2 if here else 1
            while i < n:
                if command.startswith(end, i) or (here and command.startswith("\r" + end, i)):
                    i += len(end) + (1 if command[i] == "\r" else 0)
                    break
                if not here and command[i] == "'" and command[i + 1 : i + 2] == "'":
                    word.append("'")
                    i += 2
                    continue
                if not here and command[i] == "'":
                    i += 1
                    break
                word.append(command[i])
                i += 1
            else:
                if not here:
                    raise ValueError("unterminated quote")
            has_word = True
        elif char == '"':
            i += 1
            while i < n and command[i] != '"':
                if command[i] == "`" and i + 1 < n:
                    i += 1
                word.append(command[i])
                i += 1
            if i >= n:
                raise ValueError("unterminated quote")
            i += 1
            has_word = True
        elif char == "$" and command[i + 1 : i + 2] == "{":
            end = command.find("}", i)
            end = n - 1 if end < 0 else end
            word.append(command[i : end + 1])  # ${env:TEMP}
            has_word = True
            i = end + 1
        elif (char == "$" and command[i + 1 : i + 2] == "(") or (char == "@" and command[i + 1 : i + 2] in ("(", "{")):
            flush()
            tokens.append(("(", True))
            i += 2
        elif char in ";|{}()\n" or command.startswith("&&", i):
            flush()
            op = "&&" if command.startswith("&&", i) else "||" if command.startswith("||", i) else char
            tokens.append((op, True))
            i += len(op)
        elif char in ">" or (char in "0123456789*" and not has_word and command[i + 1 : i + 2] == ">"):
            flush()
            match = re.compile(r"[0-9*]?>>?(&\d)?").match(command, i)
            op = match.group(0)
            i += len(op)
            tokens.append((">&dup" if match.group(1) else (">>" if ">>" in op else ">"), True))
        elif char == "<":
            flush()
            tokens.append(("<", True))
            i += 1
        else:
            word.append(char)
            has_word = True
            i += 1
    flush()
    return tokens


def _tokenize_cmd(command: str) -> list[tuple[str, bool]]:
    tokens, word, i, n = [], [], 0, len(command)
    has_word = False

    def flush() -> None:
        nonlocal word, has_word
        if has_word:
            tokens.append(("".join(word), False))
        word, has_word = [], False

    while i < n:
        char = command[i]
        if char in " \t\r":
            flush()
            i += 1
        elif char == "^" and i + 1 < n:
            word.append(command[i + 1])
            has_word = True
            i += 2
        elif char == '"':
            end = command.find('"', i + 1)
            end = n if end < 0 else end
            word.append(command[i + 1 : end])
            has_word = True
            i = end + 1
        elif char in "&|\n" or char in "<>" or (char in "0123456789" and not has_word and command[i + 1 : i + 2] == ">"):
            flush()
            match = re.compile(r"&&|\|\||[0-9]?>>?(&\d)?|[&|<\n]").match(command, i)
            op = match.group(0)
            i += len(op)
            if match.group(1):
                op = ">&dup"
            elif op.endswith(">"):
                op = ">>" if ">>" in op else ">"
            tokens.append((op, True))
        else:
            word.append(char)
            has_word = True
            i += 1
    flush()
    return tokens


def parse(command: str, shell: str) -> list[Segment]:
    tokenize = {"bash": _tokenize_bash, "powershell": _tokenize_powershell, "cmd": _tokenize_cmd}[shell]
    return _segments(tokenize(command))


# --- command classification ------------------------------------------------------------

REMOVALS = {"rm", "rmdir", "unlink", "shred", "del", "erase", "rd", "remove-item", "ri"}
NESTED = {"cmd": "cmd", "bash": "bash", "sh": "bash", "zsh": "bash", "dash": "bash",
          "pwsh": "powershell", "powershell": "powershell"}
EVAL = {"iex", "invoke-expression", "eval"}
CD = {"cd", "chdir", "pushd", "set-location", "sl", "push-location"}
NOOP = {
    "echo", "printf", "write-output", "write-host", "write", "true", "false", "exit", "return",
    "sleep", "start-sleep", "date", "get-date", "pwd", "get-location", "gl", "whoami", "hostname",
    "which", "where", "get-command", "gcm", "clear", "cls", "out-null", "out-host", "out-string",
    "format-table", "ft", "format-list", "fl", "select-object", "select", "sort-object",
    "where-object", "?", "foreach-object", "%", "measure-object", "measure", "group-object",
    "convertto-json", "convertfrom-json", "get-member", "gm", "set-strictmode", "popd", "pop-location",
}
READERS = {
    "cat", "less", "more", "head", "tail", "ls", "dir", "tree", "stat", "file", "wc", "du", "diff",
    "cmp", "comm", "md5sum", "sha1sum", "sha256sum", "realpath", "readlink", "type", "get-content",
    "gc", "get-childitem", "gci", "get-item", "gi", "get-itemproperty", "gp", "test-path",
    "resolve-path", "rvpa", "select-string", "sls", "get-filehash", "import-csv", "get-acl", "fc",
    "comp", "findstr", "grep", "egrep", "fgrep", "rg", "sort", "uniq", "cut", "jq", "xxd", "od",
    "hexdump", "strings", "nl", "tac", "awk", "sed", "split-path", "join-path", "find",
}
PATTERN_FIRST = {"grep", "egrep", "fgrep", "rg", "findstr", "select-string", "sls", "sed", "awk", "jq"}
PATTERN_FLAGS = {"-e", "--regexp", "-f", "--file", "-pattern", "--expression"}
WRITERS = {
    "mkdir", "md", "touch", "tee", "tee-object", "new-item", "ni", "set-content", "add-content",
    "ac", "out-file", "clear-content", "clc", "rename-item", "rni", "ren", "rename", "chmod",
    "chown", "attrib", "truncate", "export-csv", "set-acl", "mklink",
}
COPIES = {"cp", "copy", "copy-item", "cpi", "xcopy", "install", "scp", "rsync", "ln",
          "expand-archive", "compress-archive"}
MOVES = {"mv", "move", "move-item", "mi"}
WRAPPERS = {"sudo", "env", "nohup", "time", "command", "builtin", "exec", "nice", "xargs", "timeout"}
WRAPPER_VALUE_FLAGS = {"-n", "-I", "-P", "-L", "-d", "-s", "-k", "-u", "-E", "--max-args", "--max-procs"}

DEST_FLAGS = {"-destination", "-destinationpath", "-t", "--target-directory"}
PATH_FLAGS = {"-path", "-literalpath", "-lp", "-pspath", "-filepath"}
# Flags whose value is not a path. PowerShell names are compared lower-cased.
PS_VALUE_FLAGS = {
    "-value", "-inputobject", "-encoding", "-itemtype", "-type", "-name", "-newname", "-filter",
    "-include", "-exclude", "-pattern", "-erroraction", "-ea", "-warningaction", "-wa",
    "-errorvariable", "-ev", "-outvariable", "-ov", "-tail", "-head", "-totalcount", "-first",
    "-last", "-skip", "-depth", "-delimiter", "-context",
}
_GREP_VALUE_FLAGS = {"-e", "-f", "-m", "-A", "-B", "-C", "--regexp", "--file", "--max-count"}
BASH_VALUE_FLAGS = {
    "head": {"-n", "-c"},
    "tail": {"-n", "-c"},
    "grep": _GREP_VALUE_FLAGS,
    "egrep": _GREP_VALUE_FLAGS,
    "fgrep": _GREP_VALUE_FLAGS,
    "rg": _GREP_VALUE_FLAGS | {"-g", "-t", "-T", "--glob", "--type", "--type-not"},
    "sed": {"-e", "-f", "--expression", "--file"},
    "awk": {"-f", "-v", "-F"},
}
FILE_VALUE_FLAGS = {"-f", "--file"}  # for the BASH_VALUE_FLAGS commands: the value is a file that is read


def _name(token: str) -> str:
    base = re.split(r"[\\/]", token)[-1].lower()
    return re.sub(r"\.(exe|cmd|bat|com)$", "", base)


def _is_flag(token: str, shell: str) -> bool:
    if token.startswith("-") and len(token) > 1 and not re.fullmatch(r"-\d+", token):
        return True
    return shell in ("powershell", "cmd") and bool(re.fullmatch(r"/[A-Za-z?]{1,3}(:.*)?", token))


def _flag_value(token: str, shell: str) -> tuple[str, str]:
    """Split `--x=v` / PowerShell `-x:v` into (flag, value)."""
    if "=" in token:
        flag, _, value = token.partition("=")
        return flag, value
    if shell == "powershell" and ":" in token:
        flag, _, value = token.partition(":")
        return flag, value
    return token, ""


def _flag_key(flag: str, shell: str) -> str:
    return flag.lower() if shell in ("powershell", "cmd") or flag.startswith("--") else flag


@dataclass
class _Args:
    positionals: list[str] = field(default_factory=list)
    paths: list[str] = field(default_factory=list)  # values of PATH_FLAGS
    dests: list[str] = field(default_factory=list)  # values of DEST_FLAGS
    reads: list[str] = field(default_factory=list)  # values of FILE_VALUE_FLAGS
    flags: list[str] = field(default_factory=list)


def _parse_args(name: str, args: list[str], shell: str) -> _Args:
    parsed = _Args()
    value_flags = PS_VALUE_FLAGS if shell == "powershell" else BASH_VALUE_FLAGS.get(name, set())
    i = 0
    while i < len(args):
        token = args[i]
        i += 1
        if token == "--":
            parsed.positionals.extend(args[i:])
            break
        if not _is_flag(token, shell):
            if not _PIPELINE_VARS.match(token):
                parsed.positionals.append(token)
            continue
        flag, value = _flag_value(token, shell)
        key = _flag_key(flag, shell)
        parsed.flags.append(key)
        takes_value = key in PATH_FLAGS or key in DEST_FLAGS or key in value_flags
        if not value and takes_value and i < len(args):
            value = args[i]
            i += 1
        if key in PATH_FLAGS and value:
            parsed.paths.append(value)
        elif key in DEST_FLAGS and value:
            parsed.dests.append(value)
        elif key in FILE_VALUE_FLAGS and key in value_flags and value:
            parsed.reads.append(value)
    return parsed


def _path_like(token: str) -> bool:
    if not token or "\n" in token or "://" in token or _PIPELINE_VARS.match(token):
        return False
    if re.match(r"^([A-Za-z]:[\\/]|[\\/]|~[\\/]|\.\.?[\\/]|\.\.$|\$env:|\$\{)", token, re.IGNORECASE):
        return True  # absolute or explicitly relative, even with spaces: `git -C "/d/other repo"`
    if any(char.isspace() for char in token):
        return False  # prose, e.g. a commit message
    if re.match(r"^[A-Za-z]:", token) or token.startswith("~") or token in (".", ".."):
        return True
    if "/" in token or "\\" in token:
        return True
    return bool(re.match(r"^(\$env:\w|\$\{\w|%\w+%|\$\w+$)", token, re.IGNORECASE))


def _strip_prefixes(argv: list[str], shell: str) -> list[str]:
    """Drop env assignments, wrappers (sudo, xargs, ...) and PowerShell call operators."""
    argv = list(argv)
    while argv:
        head = argv[0]
        if shell == "bash" and re.match(r"^\w+=", head):
            argv.pop(0)
        elif shell == "powershell" and head.startswith("$") and len(argv) > 1 and re.fullmatch(r"[+\-*/]?=", argv[1]):
            argv = argv[2:]
        elif shell == "powershell" and re.match(r"^\$\w+[+\-*/]?=$", head):
            argv.pop(0)
        elif head in ("&", ".") and shell == "powershell":
            if len(argv) > 1 and (argv[1].startswith("$") or argv[1] in ("(", "{")):
                raise _Deny("calling a command through a variable (`& $x`) is not allowed")
            argv.pop(0)
        elif _name(head) in WRAPPERS:
            wrapper = _name(head)
            argv.pop(0)
            while argv and (argv[0].startswith("-") or (wrapper == "env" and "=" in argv[0])):
                flag = argv.pop(0)
                if flag in WRAPPER_VALUE_FLAGS and argv:
                    argv.pop(0)
            if wrapper == "timeout" and argv:
                argv.pop(0)  # the duration
        else:
            break
    return argv


def _nested_command(name: str, args: list[str]) -> tuple[str | None, str | None]:
    """(command string passed to a nested shell, its working directory), or (None, None)."""
    if name == "cmd":
        for i, arg in enumerate(args):
            if arg.lower() in ("/c", "/k", "//c", "//k"):
                return " ".join(args[i + 1 :]), None
        return None, None
    if NESTED[name] == "bash":
        for i, arg in enumerate(args):
            if re.fullmatch(r"-[a-z]*c[a-z]*", arg) and i + 1 < len(args):
                return args[i + 1], None
            if not arg.startswith("-"):
                return None, None  # `bash script.sh`
        return None, None
    skip_value = {"-executionpolicy", "-ex", "-ep", "-outputformat", "-of", "-inputformat", "-if",
                  "-configurationname", "-settingsfile", "-windowstyle", "-w"}
    workdir, i = None, 0
    while i < len(args):
        arg = args[i].lower()
        if arg in ("-e", "-ec") or (arg.startswith("-en") and "-encodedcommand".startswith(arg)) or arg.startswith("-encodedarg"):
            raise _Deny("encoded PowerShell commands are not allowed")
        if arg in ("-c", "-command") or (len(arg) > 2 and "-command".startswith(arg) and arg.startswith("-co")):
            return " ".join(args[i + 1 :]), workdir
        if arg in ("-f", "-file") or (len(arg) > 2 and "-file".startswith(arg)):
            return None, None
        if arg in ("-workingdirectory", "-wd") or (len(arg) > 2 and "-workingdirectory".startswith(arg)):
            workdir = args[i + 1] if i + 1 < len(args) else None
            i += 2
            continue
        if arg in skip_value:
            i += 2
            continue
        if not arg.startswith("-"):
            return " ".join(args[i:]), workdir
        i += 1
    return None, None


def _substitutions(token: str) -> list[str]:
    """Commands inside `$( ... )` of a token (e.g. a quoted string): the shell runs them."""
    commands, i = [], 0
    while (start := token.find("$(", i)) >= 0:
        depth, j = 1, start + 2
        while j < len(token) and depth:
            depth += {"(": 1, ")": -1}.get(token[j], 0)
            j += 1
        commands.append(token[start + 2 : j - 1 if depth == 0 else j])
        i = j
    return [command for command in commands if command.strip()]


# File-system classes only; [IO.Path] is pure string manipulation.
_DOTNET_CLASSES = r"(?:File|Directory|FileInfo|DirectoryInfo|FileStream|StreamReader|StreamWriter)"
_DOTNET_ANY = re.compile(r"\[(?:System\.)?IO\." + _DOTNET_CLASSES + r"\]::", re.IGNORECASE)
_DOTNET_CALL = re.compile(r"\[(?:System\.)?IO\.(" + _DOTNET_CLASSES + r")\]::(\w+)\s*\(([^()]*)\)", re.IGNORECASE)
_STRING = re.compile(r"'((?:[^']|'')*)'|\"([^\"]*)\"")


def _check_dotnet(command: str, ctx: Context, cwd: str) -> None:
    calls = _DOTNET_CALL.findall(command)
    if len(calls) != len(_DOTNET_ANY.findall(command)):
        raise _Deny("cannot check a .NET file API call whose arguments are not literal strings")
    for _cls, method, args in calls:
        rest = _STRING.sub("", args)
        if re.search(r"[^\s,]", re.sub(r"\$(true|false|null)\b|\b\d+\b", "", rest, flags=re.IGNORECASE)):
            raise _Deny(f"cannot check '{method}(...)': only literal string arguments are allowed")
        literals = [single.replace("''", "'") or double for single, double in _STRING.findall(args)]
        method = method.lower()
        if "delete" in method:
            raise _Deny(f".NET deletes are not allowed; {REMOVAL_HINT}")
        if not literals:
            continue
        if method.startswith("copy"):
            _require_read(literals[0], ctx, cwd, "powershell")
            for target in literals[1:2]:
                _require_write(target, ctx, cwd, "powershell")
        elif method.startswith(("move", "replace")):
            for target in literals[:2]:
                _require_write(target, ctx, cwd, "powershell")
        elif method.startswith(("read", "exists", "get", "enumerate", "openread", "opentext")):
            _require_read(literals[0], ctx, cwd, "powershell")
        else:
            _require_write(literals[0], ctx, cwd, "powershell")


_ENCODED_PIPE = re.compile(r"\bbase64\b[^|]*\s(-d|-D|--decode)\b.*\|\s*\S*(sh|bash|pwsh|powershell|cmd)\b", re.IGNORECASE)


@dataclass
class _Found:
    removal: Segment | None = None
    removal_cwd: str = ""
    nested_removal: bool = False


def _check_command(command: str, shell: str, ctx: Context, cwd: str, depth: int) -> tuple[list[Segment], _Found]:
    if "frombase64string" in command.lower() or _ENCODED_PIPE.search(command):
        raise _Deny("decoding and running encoded commands is not allowed")
    if shell == "powershell":
        _check_dotnet(command, ctx, cwd)
    segments = parse(command, shell)
    found = _Found()
    for segment in segments:
        cwd = _check_segment(segment, shell, ctx, cwd, depth, found)
    return segments, found


def _check_segment(segment: Segment, shell: str, ctx: Context, cwd: str, depth: int, found: _Found) -> str:
    """Check one segment; returns the working directory for the segments after it."""
    for token in segment.argv + [target for _, target in segment.redirects]:
        for inner in _substitutions(token):
            _, inner_found = _check_command(inner, shell, ctx, cwd, depth)
            if inner_found.removal or inner_found.nested_removal:
                found.nested_removal = True
    for op, target in segment.redirects:
        (_require_read if op == "<" else _require_write)(target, ctx, cwd, shell)
    argv = _strip_prefixes(segment.argv, shell)
    if not argv:
        return cwd
    name, args = _name(argv[0]), argv[1:]

    if name in EVAL:
        raise _Deny(f"dynamic evaluation (`{argv[0]}`) is not allowed")
    if name in NESTED:
        inner, workdir = _nested_command(name, args)
        if inner is not None:
            if depth >= 1:
                raise _Deny("shells nested more than one level deep are not allowed")
            if NESTED[name] != "powershell" and ("$(" in inner or "`" in inner):
                raise _Deny("a nested shell with command substitution is not allowed")
            inner_cwd = cwd
            if workdir:
                _require_read(workdir, ctx, cwd, shell)
                inner_cwd = resolve(workdir, ctx, cwd, shell) or cwd
            _, inner_found = _check_command(inner, NESTED[name], ctx, inner_cwd, depth + 1)
            if inner_found.removal or inner_found.nested_removal:
                found.nested_removal = True
            return cwd
    if name in REMOVALS:
        if found.removal:
            found.nested_removal = True  # two removals: not a standalone one
        found.removal, found.removal_cwd = segment, cwd
        return cwd
    if name == "robocopy" and any(arg.lower() in ("/mir", "/purge", "/mov", "/move") for arg in args):
        raise _Deny(f"robocopy flags that delete files are not allowed; {REMOVAL_HINT}")
    if name == "find":
        return _check_find(args, ctx, cwd, shell, found)

    parsed = _parse_args(name, args, shell)
    for token in parsed.reads:
        _require_read(token, ctx, cwd, shell)
    if name in CD:
        targets = parsed.positionals + parsed.paths
        if not targets or targets[0] == "-":
            return cwd
        _require_read(targets[0], ctx, cwd, shell)
        return resolve(targets[0], ctx, cwd, shell) or cwd
    if name in NOOP:
        return cwd
    if name in READERS:
        positionals = parsed.positionals
        if name == "sed" and any(flag.startswith(("-i", "--in-place")) for flag in parsed.flags):
            files = positionals[0 if "-e" in parsed.flags or "--expression" in parsed.flags else 1 :]
            for token in files + parsed.paths:
                _require_write(token, ctx, cwd, shell)
            return cwd
        if name in PATTERN_FIRST and not PATTERN_FLAGS & set(parsed.flags):
            positionals = positionals[1:]
        for token in positionals + parsed.paths:
            _require_read(token, ctx, cwd, shell)
        return cwd
    if name in WRITERS:
        for token in parsed.positionals + parsed.paths + parsed.dests:
            _require_write(token, ctx, cwd, shell)
        return cwd
    if name in COPIES or name in MOVES or name == "robocopy":
        operands = parsed.positionals + parsed.paths
        dests = parsed.dests
        if name == "robocopy":
            operands, dests = operands[:1], operands[1:2]
        elif not dests and len(operands) > 1:
            operands, dests = operands[:-1], operands[-1:]
        for token in operands:
            (_require_write if name in MOVES else _require_read)(token, ctx, cwd, shell)
        for token in dests:
            _require_write(token, ctx, cwd, shell)
        return cwd

    # Unknown command: any path argument could be written to.
    for token in args:
        value = _flag_value(token, shell)[1] if _is_flag(token, shell) else token
        if _path_like(value):
            _require_write(value, ctx, cwd, shell)
    return cwd


def _check_find(args: list[str], ctx: Context, cwd: str, shell: str, found: _Found) -> str:
    i = 0
    while i < len(args) and not args[i].startswith(("-", "(", "!")):
        _require_read(args[i], ctx, cwd, shell)
        i += 1
    expression = args[i:]
    if "-delete" in expression:
        raise _Deny(f"`find -delete` is not allowed; {REMOVAL_HINT}")
    for j, token in enumerate(expression):
        if token in ("-exec", "-execdir", "-ok", "-okdir") and j + 1 < len(expression):
            if _name(expression[j + 1]) in REMOVALS:
                raise _Deny(f"`find -exec rm` is not allowed; {REMOVAL_HINT}")
    return cwd


# --- Rewrite ---------------------------------------------------------------------------

_BASH_RM_FLAG = re.compile(r"-[rRfvdI]+|--(recursive|force|verbose|dir|interactive=never)")
_PS_RM_SWITCHES = {"-recurse": 2, "-force": 3, "-verbose": 2, "-confirm": 3}


def _removal_targets(segment: Segment, shell: str) -> list[str] | None:
    """Path tokens of a Removal, or None when its flags can't be rewritten safely."""
    argv = _strip_prefixes(segment.argv, shell)
    args = argv[1:]
    targets, i = [], 0
    while i < len(args):
        token = args[i]
        i += 1
        if token == "--":
            targets.extend(args[i:])
            break
        if not _is_flag(token, shell):
            targets.append(token)
            continue
        if shell == "bash":
            if not _BASH_RM_FLAG.fullmatch(token):
                return None
            continue
        flag, value = _flag_value(token, shell)
        flag = flag.lower()
        if any(len(flag) >= minimum and full.startswith(flag) for full, minimum in _PS_RM_SWITCHES.items()):
            continue
        if flag in ("-path", "-p", "-literalpath", "-lp", "-pspath"):
            value = value or (args[i] if i < len(args) else "")
            i += 0 if ":" in token else 1
            targets.append(value)
        elif flag in ("-erroraction", "-ea", "-err", "-erro"):
            i += 0 if ":" in token else 1
        else:
            return None
    return targets


def session_trash(ctx: Context) -> str:
    """Trash folder of this session: TRASH\\<Workspace leaf name>\\<session id>."""
    leaf = os.path.basename(os.path.normpath(ctx.workspace)) or "workspace"
    session = re.sub(r"[^\w.-]", "_", ctx.session_id or "") or "unknown-session"
    return os.path.join(TRASH, leaf, session)


def trash_path(source: str, ctx: Context) -> str:
    """Where a removed item goes: its original path, without the drive, under session_trash()."""
    return os.path.join(session_trash(ctx), os.path.splitdrive(source)[1].lstrip("\\/"))


def _clear_trash_copy(dest: str) -> None:
    """A second removal of the same path overwrites the older copy in Trash."""
    if not in_trash(dest) or not os.path.lexists(dest):
        return
    if os.path.isdir(dest) and not os.path.islink(dest):
        shutil.rmtree(dest)
    else:
        os.remove(dest)


def _quote_bash(path: str) -> str:
    return "'" + path.replace("\\", "/").replace("'", "'\\''") + "'"


def _quote_ps(text: str) -> str:
    return "'" + text.replace("'", "''") + "'"


def _rewrite(segment: Segment, shell: str, ctx: Context, cwd: str) -> Decision:
    tokens = _removal_targets(segment, shell)
    if tokens is None or not tokens:
        return _deny(f"this removal can't be rewritten (unsupported flags or no paths); {REMOVAL_HINT}")
    sources: list[str] = []
    for token in tokens:
        path = resolve(token, ctx, cwd, shell)
        if path is None:
            continue
        matches = sorted(glob.glob(path)) if re.search(r"[*?\[]", token) else []
        if not matches:
            matches = [path]  # a literal name, or a name that only looks like a wildcard
        for match in matches:
            match = os.path.normpath(match)
            if in_trash(match):
                raise _Deny("removing anything inside D:\\.slop-trash is not allowed; only the user empties Trash")
            if not in_write_zone(match, ctx):
                raise _Deny(f"removing '{token}' is not allowed: {WRITE_ZONE_TEXT}")
            if os.path.normcase(match) == os.path.normcase(os.path.normpath(ctx.workspace)):
                raise _Deny("removing the Workspace itself is not allowed")
            if os.path.lexists(match) and match not in sources:
                sources.append(match)

    if not sources:
        message = PREFIX + "nothing to remove"
        command = f"echo {_quote_bash(message)}" if shell == "bash" else f"Write-Output {_quote_ps(message)}"
        return Decision("rewrite", PREFIX + "removal targets do not exist; replaced with a no-op", command)

    message = f"{PREFIX}moved {len(sources)} item(s) to Trash under {session_trash(ctx)}"
    steps = []
    for source in sources:
        dest = trash_path(source, ctx)
        _clear_trash_copy(dest)
        # The rewritten command holds no removal of its own: agents see it and may send it again.
        parent = os.path.dirname(dest)
        if shell == "bash":
            steps.append(f"mkdir -p {_quote_bash(parent)} && mv -f -- {_quote_bash(source)} {_quote_bash(dest)}")
        else:
            steps.append(
                f"$null = New-Item -ItemType Directory -Force -Path {_quote_ps(parent)}; "
                f"Move-Item -LiteralPath {_quote_ps(source)} -Destination {_quote_ps(dest)}"
            )
    if shell == "bash":
        command = " && ".join(steps + [f"echo {_quote_bash(message)}"])
    else:
        command = "$ErrorActionPreference = 'Stop'; " + "; ".join(steps + [f"Write-Output {_quote_ps(message)}"])
    return Decision("rewrite", PREFIX + "removal rewritten into a move to D:\\.slop-trash", command)


# --- entry point for shell tools -------------------------------------------------------


def check_shell(command: str, shell: str, ctx: Context) -> Decision:
    """Check a bash or PowerShell tool command; Removals come back as a Rewrite."""
    try:
        segments, found = _check_command(command, shell, ctx, ctx.cwd, depth=0)
        if found.nested_removal:
            return _deny(REMOVAL_HINT)
        if not found.removal:
            return ALLOW
        if len(segments) != 1 or found.removal.redirects:
            return _deny(REMOVAL_HINT)
        return _rewrite(found.removal, shell, ctx, found.removal_cwd)
    except _Deny as denied:
        return _deny(str(denied))
    except ValueError as error:
        return _deny(f"could not parse the command ({error}); simplify it")
