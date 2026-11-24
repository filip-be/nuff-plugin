"""nuff-pulse Claude-side probe (milestone 1c).

Reads Claude Code's local files and prints an anonymized report:

- schemas of recent transcripts (`<root>/projects/<enc-cwd>/<id>.jsonl`): per
  record type, every key path with its value types and counts, never values
  (except for an allowlist of enum-like fields such as `type` or `entrypoint`);
- schemas of `<root>/sessions/<pid>.json` and whether each pid is alive;
- `entrypoint` values per transcript;
- what is written between a `tool_use` and its `tool_result`, to answer
  whether approving a permission prompt leaves a transcript entry before the
  tool finishes.

Optional outputs:

  --fixtures DIR   writes sanitized transcript samples (strings replaced,
                   ids remapped, enum fields kept) for widget tests.
  --watch FILE     tails one transcript and prints record type, timestamp and
                   tool ids live; approve a prompt in that session and watch.

Never reads `*.key` files. Run with `py -3 -I probe.py` (Windows) or
`python3 -I probe.py`.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import sys
import time
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path

# Field names whose string values are kept: they are enums or model names,
# not user content.
KEEP_VALUES = {
    "type", "subtype", "entrypoint", "userType", "role", "model", "stop_reason",
    "operation", "kind", "status", "nameSource", "level",
    "permissionDecision", "toolDenialKind", "turnOrigin", "turnPosition",
    "origin", "querySource", "effort", "perTurnEffort", "requestedModel",
    "clientPlatform", "renderedRole", "service_tier", "speed",
    "thinkingDisplay", "madeBy", "reason", "version",
}
# Kept only inside the `name` key of tool_use content blocks.
TOOL_NAME_PATH_SUFFIX = ("message", "content", "[]", "name")
# String fields that hold ids: remapped consistently in fixtures.
ID_KEYS = {
    "uuid", "parentUuid", "sessionId", "leafUuid", "messageId", "requestId",
    "id", "tool_use_id", "promptId", "sourceToolAssistantUUID", "hostSessionId",
    "commandUuid", "deliveryId", "requestRef",
}
TIME_KEYS = {"timestamp"}
MAX_ENUM_VALUES = 40


def claude_root() -> Path:
    env = os.environ.get("CLAUDE_CONFIG_DIR")
    return Path(env) if env else Path.home() / ".claude"


def type_name(v) -> str:
    if v is None:
        return "null"
    if isinstance(v, bool):
        return "bool"
    if isinstance(v, int):
        return "int"
    if isinstance(v, float):
        return "float"
    if isinstance(v, str):
        return "str"
    if isinstance(v, list):
        return "list"
    return "obj"


def record_kind(rec: dict) -> str:
    t = rec.get("type", "?")
    sub = rec.get("subtype")
    return f"{t}/{sub}" if isinstance(sub, str) else str(t)


class Schema:
    """Key paths → value type counts, plus allowlisted enum values."""

    def __init__(self):
        self.types: dict[str, Counter] = defaultdict(Counter)
        self.enums: dict[str, Counter] = defaultdict(Counter)
        self.records = 0

    def add(self, rec, path=()):
        if path == ():
            self.records += 1
        key = ".".join(path) or "<root>"
        self.types[key][type_name(rec)] += 1
        if isinstance(rec, dict):
            for k, v in rec.items():
                self.add(v, path + (k,))
        elif isinstance(rec, list):
            for item in rec:
                self.add(item, path + ("[]",))
        elif isinstance(rec, str) and path:
            if path[-1] in KEEP_VALUES or path[-4:] == TOOL_NAME_PATH_SUFFIX:
                e = self.enums[key]
                if rec in e or len(e) < MAX_ENUM_VALUES:
                    e[rec] += 1

    def to_json(self) -> dict:
        return {
            "records": self.records,
            "paths": {k: dict(v) for k, v in sorted(self.types.items())},
            "values": {k: dict(v.most_common()) for k, v in sorted(self.enums.items())},
        }


def read_jsonl(path: Path):
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(rec, dict):
                yield rec


def recent_transcripts(root: Path, days: float, limit: int) -> list[Path]:
    cutoff = time.time() - days * 86400
    files = [
        Path(p) for p in glob.glob(str(root / "projects" / "*" / "*.jsonl"))
        if os.path.getmtime(p) >= cutoff
    ]
    files.sort(key=os.path.getmtime, reverse=True)
    return files[:limit]


def parse_ts(v) -> float | None:
    if isinstance(v, (int, float)):
        return v / 1000 if v > 1e11 else float(v)
    if isinstance(v, str):
        try:
            return datetime.fromisoformat(v.replace("Z", "+00:00")).timestamp()
        except ValueError:
            return None
    return None


def content_blocks(rec: dict) -> list[dict]:
    msg = rec.get("message")
    if isinstance(msg, dict) and isinstance(msg.get("content"), list):
        return [b for b in msg["content"] if isinstance(b, dict)]
    return []


def between_tool_calls(records: list[dict]) -> dict:
    """What is written between a tool_use and its tool_result.

    For each main-thread tool_use, counts the record kinds that appear after
    it and before its tool_result, and the fields present on the user record
    that carries the tool_result (e.g. `permissionDecision`).
    """
    open_calls: dict[str, tuple[int, str, float | None]] = {}
    between: Counter = Counter()
    between_by_tool: dict[str, Counter] = defaultdict(Counter)
    result_fields: Counter = Counter()
    decisions: Counter = Counter()
    gaps: dict[str, list[float]] = defaultdict(list)
    for i, rec in enumerate(records):
        if rec.get("isSidechain"):
            continue
        for b in content_blocks(rec):
            if b.get("type") == "tool_use" and isinstance(b.get("id"), str):
                open_calls[b["id"]] = (i, str(b.get("name")), parse_ts(rec.get("timestamp")))
            elif b.get("type") == "tool_result" and b.get("tool_use_id") in open_calls:
                start, name, t0 = open_calls.pop(b["tool_use_id"])
                for mid in records[start + 1:i]:
                    if mid.get("isSidechain"):
                        continue
                    kind = record_kind(mid)
                    if kind == "attachment" and isinstance(mid.get("attachment"), dict):
                        kind += f"/{mid['attachment'].get('type')}"
                    between[kind] += 1
                    between_by_tool[name][kind] += 1
                for k in rec:
                    result_fields[k] += 1
                d = rec.get("permissionDecision")
                decisions[f"{name}: {json.dumps(d)[:60]}"] += 1
                t1 = parse_ts(rec.get("timestamp"))
                if t0 is not None and t1 is not None:
                    gaps[name].append(round(t1 - t0, 1))
    return {
        "record_kinds_between_use_and_result": dict(between.most_common()),
        "by_tool": {k: dict(v.most_common()) for k, v in sorted(between_by_tool.items())},
        "tool_result_record_fields": dict(result_fields.most_common()),
        "permissionDecision_values": dict(decisions.most_common(40)),
        "seconds_use_to_result_max": {k: max(v) for k, v in sorted(gaps.items())},
        "unmatched_tool_uses": len(open_calls),
    }


def pid_alive(pid: int) -> bool | None:
    if sys.platform == "win32":
        import ctypes
        from ctypes import wintypes

        k32 = ctypes.WinDLL("kernel32", use_last_error=True)
        k32.OpenProcess.restype = wintypes.HANDLE
        k32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        h = k32.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
        if not h:
            return False
        code = wintypes.DWORD()
        ok = k32.GetExitCodeProcess(h, ctypes.byref(code))
        k32.CloseHandle(h)
        return bool(ok) and code.value == 259  # STILL_ACTIVE
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    except OSError:
        return None


def probe_sessions(root: Path) -> dict:
    schema = Schema()
    live = Counter()
    transcript_match = Counter()
    for p in glob.glob(str(root / "sessions" / "*.json")):
        try:
            with open(p, encoding="utf-8") as f:
                d = json.load(f)
        except (OSError, json.JSONDecodeError):
            continue
        if not isinstance(d, dict):
            continue
        schema.add(d)
        pid = d.get("pid")
        if isinstance(pid, int):
            live[f"file pid alive={pid_alive(pid)}"] += 1
            live[f"file name matches pid={Path(p).stem == str(pid)}"] += 1
        sid, cwd = d.get("sessionId"), d.get("cwd")
        if isinstance(sid, str):
            found = glob.glob(str(root / "projects" / "*" / f"{sid}.jsonl"))
            transcript_match[f"transcript found={bool(found)}"] += 1
            if found and isinstance(cwd, str):
                enc = Path(found[0]).parent.name
                transcript_match[f"folder == encoded cwd={enc == encode_cwd(cwd)}"] += 1
    return {"schema": schema.to_json(), "liveness": dict(live), "transcripts": dict(transcript_match)}


def encode_cwd(cwd: str) -> str:
    return "".join(c if c.isalnum() or c == "-" else "-" for c in cwd)


class Sanitizer:
    def __init__(self):
        self.ids: dict[str, str] = {}
        self.t0: float | None = None

    def fake_id(self, v: str) -> str:
        if v not in self.ids:
            self.ids[v] = f"id-{len(self.ids) + 1:04d}"
        return self.ids[v]

    def clean(self, v, path=()):
        if isinstance(v, dict):
            return {k: self.clean(x, path + (k,)) for k, x in v.items()}
        if isinstance(v, list):
            return [self.clean(x, path + ("[]",)) for x in v]
        if not isinstance(v, str) or not path:
            return v
        key = path[-1]
        if key in KEEP_VALUES or path[-4:] == TOOL_NAME_PATH_SUFFIX:
            return v
        if key in ID_KEYS:
            return self.fake_id(v)
        if key in TIME_KEYS:
            t = parse_ts(v)
            if t is None:
                return v
            if self.t0 is None:
                self.t0 = t
            shifted = 1764000000 + (t - self.t0)
            return datetime.fromtimestamp(shifted, timezone.utc).isoformat().replace("+00:00", "Z")
        if key == "cwd":
            return "C:\\work\\project"
        if key == "gitBranch":
            return "main" if v in ("main", "master", "HEAD", "") else "feature"
        return f"<str:{len(v)}>"


def write_fixtures(files: list[Path], out: Path, count: int) -> list[str]:
    out.mkdir(parents=True, exist_ok=True)
    written = []
    for n, p in enumerate(files[:count], 1):
        s = Sanitizer()
        target = out / f"claude-transcript-{n:02d}.jsonl"
        with open(target, "w", encoding="utf-8", newline="\n") as f:
            for rec in read_jsonl(p):
                f.write(json.dumps(s.clean(rec), ensure_ascii=False) + "\n")
        written.append(str(target))
    return written


def watch(path: Path) -> None:
    print(f"watching {path.name}; Ctrl+C to stop", flush=True)
    pos = path.stat().st_size
    buf = b""
    while True:
        size = path.stat().st_size
        if size < pos:
            pos, buf = 0, b""
        if size > pos:
            with open(path, "rb") as f:
                f.seek(pos)
                chunk = f.read(size - pos)
            pos = size
            buf += chunk
            *lines, buf = buf.split(b"\n")
            for line in lines:
                try:
                    rec = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if not isinstance(rec, dict):
                    continue
                tools = [
                    f"{b.get('type')}:{b.get('name') or ''}:{str(b.get('id') or b.get('tool_use_id'))[-6:]}"
                    for b in content_blocks(rec) if b.get("type") in ("tool_use", "tool_result")
                ]
                extra = ""
                if "permissionDecision" in rec:
                    extra = f" permissionDecision={json.dumps(rec['permissionDecision'])[:80]}"
                if isinstance(rec.get("attachment"), dict):
                    extra += f" attachment={rec['attachment'].get('type')}"
                now = datetime.now().strftime("%H:%M:%S.%f")[:-3]
                print(f"{now} written | {rec.get('timestamp', '-')} {record_kind(rec)} {' '.join(tools)}{extra}", flush=True)
        time.sleep(0.2)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--days", type=float, default=7, help="transcripts modified within N days (default 7)")
    ap.add_argument("--limit", type=int, default=50, help="max transcripts to read (default 50)")
    ap.add_argument("--out", type=Path, help="write the report here instead of stdout")
    ap.add_argument("--fixtures", type=Path, help="write sanitized transcript samples to this folder")
    ap.add_argument("--fixture-count", type=int, default=3)
    ap.add_argument("--watch", type=Path, help="tail one transcript live")
    args = ap.parse_args(argv)

    if args.watch:
        try:
            watch(args.watch)
        except KeyboardInterrupt:
            pass
        return 0

    root = claude_root()
    files = recent_transcripts(root, args.days, args.limit)
    by_kind: dict[str, Schema] = defaultdict(Schema)
    entrypoints: Counter = Counter()
    per_file_entry: Counter = Counter()
    between_all = []
    for p in files:
        records = list(read_jsonl(p))
        eps = set()
        for rec in records:
            by_kind[record_kind(rec)].add(rec)
            ep = rec.get("entrypoint")
            if isinstance(ep, str):
                entrypoints[ep] += 1
                eps.add(ep)
        per_file_entry[",".join(sorted(eps)) or "<none>"] += 1
        between_all.append(between_tool_calls(records))

    merged: dict = {"record_kinds_between_use_and_result": Counter(), "tool_result_record_fields": Counter(),
                    "permissionDecision_values": Counter(), "unmatched_tool_uses": 0}
    for b in between_all:
        for k in ("record_kinds_between_use_and_result", "tool_result_record_fields", "permissionDecision_values"):
            merged[k].update(b[k])
        merged["unmatched_tool_uses"] += b["unmatched_tool_uses"]

    report = {
        "generated": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "root_is_default": "CLAUDE_CONFIG_DIR" not in os.environ,
        "transcripts_read": len(files),
        "entrypoint_records": dict(entrypoints.most_common()),
        "entrypoints_per_transcript": dict(per_file_entry.most_common()),
        "tool_use_to_result": {k: dict(v.most_common()) if isinstance(v, Counter) else v for k, v in merged.items()},
        "sessions_dir": probe_sessions(root),
        "record_schemas": {k: v.to_json() for k, v in sorted(by_kind.items())},
    }
    if args.fixtures:
        report["fixtures_written"] = [Path(p).name for p in write_fixtures(files, args.fixtures, args.fixture_count)]

    text = json.dumps(report, indent=1, ensure_ascii=False)
    if args.out:
        args.out.write_text(text + "\n", encoding="utf-8")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
