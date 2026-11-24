# nuff-pulse-copilot

Hooks for **GitHub Copilot only** that record to a local file when a session's turn is done and when the session ends. They run in every Copilot host that loads Copilot CLI plugins: Copilot CLI (Windows and WSL), the GitHub Copilot app and VS Code's Copilot agent host. They use Copilot's hook format and do not run in other agents.

The plugin only writes the file. The reader is the **nuff-pulse** widget, a small Windows app that lists your active agent sessions; get it from the [nuff-plugin releases](https://github.com/filip-be/nuff-plugin/releases) (tags `apps/nuff-pulse/v*`). nuff-pulse reads the rest of a Copilot session's state (working, needs you, context size, prompt cache) from Copilot's own session files; the hooks add the two signals those files lack. Any other tool may read the file too: its format is documented below.

## Terms

- **Session** — one Copilot session, named by its `sessionId`.
- **Pulse File** — `~/.nuff-pulse/sessions/copilot-<session_id>.hooks.jsonl` (`%USERPROFILE%\.nuff-pulse\sessions\…` on Windows; inside WSL, the distro's own home). One file per Session.
- **Pulse Line** — one JSON object on one line of a Pulse File. Every hook call appends exactly one.
- **Agent** — the Copilot process that runs the hook: the first ancestor of the hook script that is not a launcher or shell (`py`, `python`, `bash`, `sh`, `pwsh`, `cmd`, `conhost`, …). On Windows that is `copilot.exe` (Copilot CLI) or `copilot-runtime.exe` (the app and VS Code), which several app sessions can share.

## What it does

| Hook | Pulse Line `state` | `reason` |
| ---- | ------------------ | -------- |
| `agentStop` | `done`; `error` when `stopReason` is `error` | `stopReason` (`end_turn`, `abort`, …) |
| `sessionEnd` | `ended` | `reason` (`complete`, `user_exit`, …) |

- Only these two hooks, because Copilot hooks block the agent: each is one short Python call with a 5 s timeout.
- On Windows the hook runs through PowerShell (`pwsh`), in WSL and on Linux through Bash.
- A resumed session fires the hooks again with the same `sessionId`; a reader applies the lines in time order.
- A session killed outright fires no `sessionEnd`; a reader needs its own liveness check.
- Hooks never print, never block and always exit 0; a failure only loses that Pulse Line.

## Pulse Line format

```json
{
  "v": 1,
  "t": 1764000000.123,
  "runtime": "copilot",
  "session_id": "08bb40d5-110c-4ba2-a302-3277518a075a",
  "event": "agentStop",
  "state": "done",
  "reason": "end_turn",
  "cwd": "D:\\work\\shop",
  "log_path": "C:\\Users\\me\\.copilot\\session-state\\08bb40d5-…\\events.jsonl",
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
| `t` | The hook's `timestamp` (Unix seconds); when the hook input has none, when the hook started |
| `runtime` | Always `copilot` |
| `session_id` | The Session; letters, digits, `-` and `_` only |
| `event` | `agentStop` or `sessionEnd` |
| `state` | `done`, `error` or `ended` |
| `reason` | `stopReason` or the end `reason`, a short word; optional |
| `cwd` | The session's working directory |
| `log_path` | The session's `events.jsonl` (`transcriptPath`); only on `agentStop` |
| `host.os` | `windows`, `wsl` or `linux` (other systems: Python's `sys.platform`) |
| `host.distro` | The WSL distro name (`WSL_DISTRO_NAME`); only when `os` is `wsl` |
| `host.agent_pid` | The Agent's process id (a Linux pid inside WSL) |
| `host.agent_started` | The Agent's start time, Unix seconds; with `agent_pid` it tells a live Agent from a reused pid |
| `host.pids` | The Agent and its ancestors, nearest first, at most 16 |

Readers should treat every field except `v`, `t`, `runtime` and `session_id` as optional, ignore unknown fields (a `detail` text may appear in later versions) and skip lines that are not valid JSON.

- A line is at most 4,000 bytes (UTF-8): when it would be longer, `log_path`, then `cwd`, then `host` are left out.
- A line is written with a single append under a short file lock (`msvcrt.locking` on Windows, `flock` elsewhere), so concurrent hooks never mix their lines. When the lock is not free within 0.5 s, that line is dropped.
- The plugin never deletes Pulse Files; nuff-pulse removes files of sessions that ended more than 24 hours ago.
- Process facts come from Toolhelp32 on Windows and `/proc` on Linux; never WMI.

## Prerequisites

- Python 3.10+: on Windows the `py` launcher (`py -3 --version`), on Linux and macOS `python3` on `PATH`. Without Python the hooks do nothing.
- On Windows, PowerShell 7 (`pwsh`), which Copilot uses to run hooks.
