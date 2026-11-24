# nuff-pulse-cc

Hooks for **Claude Code only** that record each session's state to a local file: working, needs you, done, error or ended. They use Claude Code's hook format and do not run in other agents.

The plugin only writes the file. The reader is the **nuff-pulse** widget, a small Windows app that lists your active agent sessions; get it from the [nuff-plugin releases](https://github.com/filip-be/nuff-plugin/releases) (tags `apps/nuff-pulse/v*`). Any other tool may read the file too: its format is documented below.

## Terms

- **Session** — one Claude Code session, named by its `session_id`.
- **Pulse File** — `~/.nuff-pulse/sessions/claude-<session_id>.hooks.jsonl` (`%USERPROFILE%\.nuff-pulse\sessions\…` on Windows; inside WSL, the distro's own home). One file per Session.
- **Pulse Line** — one JSON object on one line of a Pulse File. Every hook call appends exactly one.
- **Agent** — the Claude Code process that runs the hook: the first ancestor of the hook script that is not a launcher or shell (`py`, `python`, `bash`, `sh`, `pwsh`, `cmd`, `conhost`, …).

## What it does

| Hook | Pulse Line `state` | `reason` / `detail` |
| ---- | ------------------ | ------------------- |
| `SessionStart` | none (seed only) | `reason`: `startup`, `resume`, `clear`, `compact`, `fork` |
| `UserPromptSubmit` | `working` | |
| `PreToolUse` on `AskUserQuestion` | `needs_you` | `question`; `detail`: the first question |
| `PostToolUse` on `AskUserQuestion` | `working` | |
| `PermissionRequest` | `needs_you` | `permission_prompt`; `detail`: `Allow <tool>: <main argument>` |
| `Notification` `permission_prompt` | `needs_you` | `permission_prompt`; `detail`: the notification text |
| `Notification` `elicitation_dialog` | `needs_you` | `elicitation`; `detail`: the notification text |
| `Stop` | `done` | |
| `StopFailure` | `error` | the error type (`rate_limit`, `server_error`, …); `detail`: its details |
| `SessionEnd` | `ended` | the end reason (`clear`, `logout`, `prompt_input_exit`, …) |

- All hooks run `async` (in the background) except `SessionEnd`, which Claude Code runs before it exits, within its 1.5 s budget.
- No hook runs on other tool calls. Claude Code fires no hook when you approve a permission prompt, so a reader learns about the approval from elsewhere (for nuff-pulse: the session's transcript and `~/.claude/sessions/<pid>.json`).
- Hooks never print, never block and always exit 0; a failure only loses that Pulse Line.

## Pulse Line format

```json
{
  "v": 1,
  "t": 1764000000.123,
  "runtime": "claude",
  "session_id": "6f1c…",
  "event": "PermissionRequest",
  "state": "needs_you",
  "reason": "permission_prompt",
  "detail": "Allow Bash: dotnet ef database update",
  "cwd": "D:\\work\\shop",
  "log_path": "C:\\Users\\me\\.claude\\projects\\D--work-shop\\6f1c….jsonl",
  "host": {
    "os": "windows",
    "pids": [1234, 5678, 910],
    "agent_pid": 1234,
    "agent_started": 1763999000.0
  }
}
```

| Field | Meaning |
| ----- | ------- |
| `v` | Format version, `1` |
| `t` | When the hook started, Unix seconds |
| `runtime` | Always `claude` |
| `session_id` | The Session; letters, digits, `-` and `_` only |
| `event` | The hook event name |
| `state` | `working`, `needs_you`, `done`, `error` or `ended`; absent on `SessionStart` |
| `reason` | A short word qualifying the state (see the table above); optional |
| `detail` | Human-readable text, one line, at most 200 characters; optional |
| `cwd` | The session's working directory |
| `log_path` | The session's transcript file |
| `host.os` | `windows`, `wsl` or `linux` (other systems: Python's `sys.platform`) |
| `host.distro` | The WSL distro name (`WSL_DISTRO_NAME`); only when `os` is `wsl` |
| `host.agent_pid` | The Agent's process id (a Linux pid inside WSL) |
| `host.agent_started` | The Agent's start time, Unix seconds; with `agent_pid` it tells a live Agent from a reused pid |
| `host.pids` | The Agent and its ancestors, nearest first, at most 16 |

Readers should treat every field except `v`, `t`, `runtime` and `session_id` as optional, ignore unknown fields and skip lines that are not valid JSON.

- `cwd`, `log_path` and the `host` facts are repeated on every line, so the newest line alone is enough.
- A line is at most 4,000 bytes (UTF-8): when it would be longer, `detail`, then `log_path`, then `cwd`, then `host` are left out.
- A line is written with a single append under a short file lock (`msvcrt.locking` on Windows, `flock` elsewhere), so concurrent hooks never mix their lines. When the lock is not free within 0.5 s, that line is dropped.
- The plugin never deletes Pulse Files; nuff-pulse removes files of sessions that ended more than 24 hours ago.
- Process facts come from Toolhelp32 on Windows and `/proc` on Linux; never WMI.

## Prerequisites

- Python 3.10+: on Windows the `py` launcher (`py -3 --version`), on Linux and macOS `python3` on `PATH`. The hook tries `py`, then `python3`; without either it does nothing.
- On Windows, Git Bash or PowerShell 7 (`pwsh`): Claude Code runs hook commands in Git Bash when it is installed, else in PowerShell. The command does not run in Windows PowerShell 5.1.
