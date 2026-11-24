"""Read-only Copilot metadata diagnostics. Run with py -3 -I."""

import argparse
import bisect
import json
import ntpath
import re
import runpy
import sys
from collections import Counter, defaultdict
from datetime import datetime
from pathlib import Path
from statistics import median

STATE = Path.home() / ".copilot" / "session-state"
LOGS = Path.home() / ".nuff-pulse-probe"
KEY = re.compile(r"[A-Za-z_][A-Za-z_0-9]{0,63}")
SESSION = re.compile(r"[0-9a-fA-F]{8}-(?:[0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12}")
OPAQUE = set("args arguments toolargs input content text prompt title response reasoningtext "
             "reasoningsummary requestmessages last_assistant_message".split())
DYNAMIC = set("agentMetrics modelMetrics models metadata headers env toolCounts".split())
USAGE = ("session.usage_record", "model.model_call_success")
CHECKPOINT = ("session.usage_checkpoint", "usage_checkpoint")
CONTEXT = ("session.compaction_start", "session.compaction_complete", "compaction_complete",
           "session.shutdown")


def emit(**row):
    print(json.dumps(row, separators=(",", ":")))


def mapping(value):
    return value if isinstance(value, dict) else {}


def number(value):
    return value if type(value) in (int, float) and 0 <= value < 1e16 else None


def timestamp(value):
    n = number(value)
    if n is not None:
        return int(n * 1000 if n < 1e11 else n)
    if isinstance(value, str):
        try:
            stamp = datetime.fromisoformat(value.replace("Z", "+00:00"))
            if stamp.tzinfo:
                return int(stamp.timestamp() * 1000)
        except (ValueError, OverflowError, OSError):
            pass
    return None


def read_jsonl(path, errors, *, optional=False):
    try:
        with path.open(encoding="utf-8-sig", errors="replace") as stream:
            for line in stream:
                try:
                    record = json.loads(line)
                except ValueError:
                    errors["invalid_lines"] += 1
                    continue
                if isinstance(record, dict):
                    yield record
                else:
                    errors["invalid_lines"] += 1
    except FileNotFoundError:
        if not optional:
            errors["unreadable_files"] += 1
    except OSError:
        errors["unreadable_files"] += 1


def sessions(limit=None):
    files = []
    for path in STATE.glob("*/events.jsonl"):
        try:
            files.append((path.stat().st_mtime_ns, path))
        except FileNotFoundError:
            continue
    return [path for _, path in sorted(files, reverse=True)[:limit]]


def schema(value, path="data", key=""):
    yield path, {dict: "object", list: "array", str: "string", bool: "boolean",
                 int: "integer", float: "number", type(None): "null"}[type(value)]
    if key.lower() in OPAQUE:
        return
    if isinstance(value, dict):
        for k, v in value.items():
            name = k if key not in DYNAMIC and KEY.fullmatch(k) else "<key>"
            yield from schema(v, f"{path}.{name}", k)
    elif isinstance(value, list):
        for v in value:
            yield from schema(v, f"{path}[]", key)


def main_thread(event):
    return "agentId" not in event and "parentToolCallId" not in mapping(event.get("data"))


def token_counts(event_type, data):
    if event_type == USAGE[0]:
        usage = mapping(data.get("usage"))
        return {k: number(usage.get(field)) for k, field in
                (("input", "inputTokens"), ("cacheRead", "cacheReadTokens"),
                 ("cacheWrite", "cacheWriteTokens"))}
    details = mapping(data.get("copilotUsage")).get("token_details")
    by_kind = defaultdict(list)
    for item in details if isinstance(details, list) else []:
        item = mapping(item)
        kind = item.get("token_type")
        count = number(item.get("token_count"))
        if kind in ("input", "cache_read", "cache_write") and count is not None:
            by_kind[kind].append(count)
    usage = mapping(mapping(data.get("responseChunk")).get("usage"))
    prompt = mapping(usage.get("prompt_tokens_details"))
    counts = {k: sum(by_kind[kind]) if by_kind[kind] else None for k, kind in
              (("input", "input"), ("cacheRead", "cache_read"), ("cacheWrite", "cache_write"))}
    counts.update(promptTotal=number(usage.get("prompt_tokens")),
                  promptCached=number(prompt.get("cached_tokens")),
                  promptCacheWrite=number(prompt.get("cache_write_tokens")),
                  promptCacheCreation=number(prompt.get("cache_creation_tokens")))
    return counts


CONTEXT_FIELDS = {
    "session.compaction_start": ("currentTokens", "preCompactionTokens", "tokensBefore"),
    "session.compaction_complete": ("postCompactionTokens", "currentTokens", "tokensAfter"),
    "compaction_complete": ("postCompactionTokens", "currentTokens", "tokensAfter"),
    "session.shutdown": ("currentTokens",),
}


def usage_probe(files):
    errors, events = Counter(), Counter()
    shapes, fields, comparisons = defaultdict(Counter), defaultdict(Counter), defaultdict(list)
    for path in files:
        points, requests = defaultdict(list), []
        epoch = 0
        for event in read_jsonl(path, errors):
            event_type = event.get("type")
            if event_type in ("session.start", "session.resume"):
                epoch += 1
            if event_type not in USAGE + CHECKPOINT + CONTEXT:
                continue
            events[event_type] += 1
            data = mapping(event.get("data"))
            shapes[event_type].update(set(schema(data)))
            stamp = timestamp(event.get("timestamp"))
            if event_type in USAGE:
                tokens = token_counts(event_type, data)
                fields[event_type].update(k for k, v in tokens.items() if v is not None)
                fields[event_type].update(k + "Nonzero" for k, v in tokens.items() if v)
                if "cacheDetailsReported" in mapping(data.get("usage")):
                    fields[event_type]["cacheDetailsReportedTrue"] += data["usage"]["cacheDetailsReported"] is True
                if stamp is not None and main_thread(event) and "parentToolCallId" not in mapping(data.get("usage")):
                    requests.append((epoch, stamp, event_type, tokens))
            elif event_type in CONTEXT and stamp is not None:
                for field in CONTEXT_FIELDS[event_type]:
                    count = number(data.get(field))
                    if count is not None:
                        points[epoch].append((stamp, event_type, field, count))
                        break
        for group in points.values():
            group.sort()
        for epoch, stamp, event_type, tokens in requests:
            group = points[epoch]
            if not group:
                continue
            i = bisect.bisect_left(group, (stamp,))
            near = group[max(0, i - 1):i + 1]
            other, context_type, field, context = min(near, key=lambda p: abs(stamp - p[0]))
            variants = dict(tokens)
            if all(tokens.get(k) is not None for k in ("input", "cacheRead", "cacheWrite")):
                variants["inputPlusCache"] = tokens["input"] + tokens["cacheRead"] + tokens["cacheWrite"]
            for variant in ("input", "inputPlusCache", "promptTotal"):
                if variants.get(variant) is not None:
                    comparisons[event_type, context_type, field, variant].append(
                        (variants[variant] - context, abs(stamp - other)))
    for event_type in USAGE + CHECKPOINT + CONTEXT:
        emit(type=event_type, events=events[event_type], numeric_fields=dict(fields[event_type]),
             key_paths=[{"path": path, "value_type": kind, "events": n}
                        for (path, kind), n in sorted(shapes[event_type].items())])
    for (event_type, context, field, variant), rows in sorted(comparisons.items()):
        emit(type=event_type, context=context, context_field=field, variant=variant, paired=len(rows),
             same=sum(delta == 0 for delta, _ in rows), min_delta=min(delta for delta, _ in rows),
             median_delta=median(delta for delta, _ in rows), max_delta=max(delta for delta, _ in rows),
             median_abs_time_gap_ms=median(gap for _, gap in rows))
    emit(usage_summary=True, sessions=len(files), **errors)


OPEN = {
    "tool.execution_start": ("tool", "toolCallId"),
    "permission.requested": ("permission", "requestId"),
    "external_tool.requested": ("external", "requestId"),
    "subagent.started": ("subagent", "toolCallId"),
    "user_input.requested": ("ask_user", "requestId"),
    "user_input.request": ("ask_user", "requestId"),
}
CLOSE = {
    "tool.execution_complete": OPEN["tool.execution_start"],
    "permission.completed": OPEN["permission.requested"],
    "external_tool.completed": OPEN["external_tool.requested"],
    "subagent.completed": OPEN["subagent.started"],
    "subagent.failed": OPEN["subagent.started"],
    "user_input.completed": OPEN["user_input.requested"],
    "user_input.response": OPEN["user_input.requested"],
}
CONTINUES = {"assistant.turn_start", "assistant.message", "model.turn_started",
             "model.model_call_started", *OPEN}
RESET = {"session.start", "session.resume", "session.shutdown", "abort"}
DISK = ("assistant.turn_end", "model.turn_ended")
REF = ("agentStop", "session.idle")


def tool_requests(data):
    requests = data.get("toolRequests")
    return requests if isinstance(requests, list) else []


def collect_turns(events):
    turns, current = [], None
    pending, messages = set(), {}
    for index, event in enumerate(events):
        if not main_thread(event):
            continue
        t = event.get("type")
        if not isinstance(t, str):
            continue
        d = mapping(event.get("data"))
        stamp = timestamp(event.get("timestamp"))
        turn_id = d.get("turnId")
        turn_id = turn_id if isinstance(turn_id, str) else None
        # Steering is delivered within the running reply, not a new idle-to-idle turn.
        new_turn = t == "user.message" and (current is None or d.get("delivery") != "steering")
        if t in RESET or new_turn:
            if current is not None:
                current["boundary"] = stamp
                current["boundary_type"] = t
            if new_turn:
                current = {"start": stamp, "assistant.turn_end": [], "model.turn_ended": [],
                           "continued_at": -1, "boundary": None, "boundary_type": None}
                turns.append(current)
            else:
                current = None
                pending.clear()
            messages.clear()
        elif t == "assistant.message" and turn_id is not None:
            requests = tool_requests(d)
            messages[turn_id] = not requests and d.get("toolRequests") in (None, [])
            for item in requests:
                key = mapping(item).get("toolCallId")
                if isinstance(key, str):
                    pending.add(("tool", key))
                else:
                    pending.add(("tool", None))
        elif t in OPEN or t in CLOSE:
            kind, key = (OPEN if t in OPEN else CLOSE)[t]
            value = d.get(key)
            blocker = kind, value if isinstance(value, str) else None
            if t in OPEN:
                pending.add(blocker)
            elif value is not None:
                pending.discard(blocker)
        if current is None:
            continue
        if t in CONTINUES or (t == "user.message" and not new_turn):
            current["continued_at"] = index
        if stamp is not None and (t == DISK[1]
                or (t == DISK[0] and messages.get(turn_id) and not pending)):
            current[t].append((index, stamp))
    return turns


def references(path, errors):
    session = path.parent.name
    if not SESSION.fullmatch(session):
        return {}
    refs = defaultdict(list)
    for e in read_jsonl(LOGS / f"hooks-{session}.jsonl", errors, optional=True):
        stamp = timestamp(e.get("writeTimeMs"))
        keys = e.get("stdinKeys")
        if (e.get("hook") == "agentStop" and stamp is not None and isinstance(keys, list)
                and not any(key in keys for key in ("agentId", "parentAgentId", "parentToolCallId"))):
            refs["agentStop"].append(stamp)
    for e in read_jsonl(LOGS / f"ext-{session}.jsonl", errors, optional=True):
        stamp = timestamp(e.get("writeTimeMs"))
        if (e.get("type") == "session.idle" and stamp is not None
                and e.get("hasAgentId") is False and e.get("hasParentToolCallId") is False):
            refs["session.idle"].append(stamp)
    return {k: sorted(v) for k, v in refs.items()}


def score_turn(turn, ref_times):
    counts, row = Counter(), {}
    lower, upper = turn["start"], turn["boundary"]
    matched = {}
    for ref in REF:
        times = ref_times.get(ref, [])
        if lower is None or (upper is None and turn["boundary_type"] is not None):
            matched[ref] = []
        else:
            matched[ref] = times[bisect.bisect_left(times, lower):
                                 bisect.bisect_left(times, upper) if upper is not None else len(times)]
        row[ref] = matched[ref]
        counts[ref + ".reference_turns"] += bool(matched[ref])
    for signal in DISK:
        candidates = turn[signal][-1:] if signal == DISK[0] else turn[signal]
        counts[signal + ".turns_without_candidate"] += not candidates
        if signal == DISK[0]:
            counts[signal + ".online_candidates"] += len(turn[signal])
            counts[signal + ".online_false_done_disk_continued"] += sum(
                i < turn["continued_at"] for i, _ in turn[signal])
        stats = []
        for i, stamp in candidates:
            early = i < turn["continued_at"]
            counts[signal + ".candidates"] += 1
            counts[signal + ".false_done_disk_continued"] += early
            stats.append({"timestamp_ms": stamp, "disk_continued": early,
                          **{r + ".offset_ms": [stamp - ref for ref in matched[r]] for r in REF}})
        row[signal] = stats
        for ref in REF:
            if matched[ref]:
                counts[f"{signal}.{ref}.missed_done"] += not any(
                    i >= turn["continued_at"] for i, _ in candidates)
    return row, counts


def done_probe(files, summary):
    errors, total = Counter(), Counter()
    for session, path in enumerate(files, 1):
        turns = collect_turns(read_jsonl(path, errors))
        ref_times = references(path, errors)
        counts = Counter(turns=len(turns))
        for index, turn in enumerate(turns, 1):
            row, result = score_turn(turn, ref_times)
            counts.update(result)
            if not summary:
                emit(session=session, turn=index, user_timestamp_ms=turn["start"], **row)
        if not summary:
            emit(session=session, done_summary=True, **counts)
        total.update(counts)
    for ref in REF:
        if not total[ref + ".reference_turns"]:
            for signal in DISK:
                total.pop(f"{signal}.{ref}.missed_done", None)
    emit(done_summary=True, sessions=len(files), **total, **errors)


def workspace_folder(path):
    try:
        with path.open(encoding="utf-8-sig") as stream:
            fields = {}
            for line in stream:
                match = re.match(r"^(cwd|git_root):[ \t]+(.+?)\s*$", line)
                if match:
                    key, value = match.groups()
                    if value.startswith('"'):
                        value = json.loads(value)
                    elif value.startswith("'") and value.endswith("'"):
                        value = value[1:-1].replace("''", "'")
                    if isinstance(value, str) and ntpath.isabs(value):
                        fields[key] = value
            return ntpath.basename(fields.get("cwd", fields.get("git_root", "")).rstrip("\\/"))
    except (OSError, ValueError):
        return None


def inuse(path, suffix):
    for entry in sorted(path.glob(f"inuse.*.{suffix}")):
        m = re.fullmatch(rf"inuse\.([0-9]{{1,10}})\.{suffix}", entry.name)
        if m and 0 < int(m[1]) <= 0xffffffff:
            yield int(m[1]), entry


def process_probe(path, command):
    win = runpy.run_path(str(Path(__file__).with_name("copilot-probe-plugin") / "win_process.py"))
    if command == "hold":
        for pid, entry in inuse(path, "hold"):
            error = win["exclusive_read"](entry)
            alive, _ = win["process_status"](pid)
            emit(pid=pid, alive=alive, opened=error == 0, windows_error=error)
        return
    snapshot = win["snapshot"]()
    folder = workspace_folder(path / "workspace.yaml")
    for pid, _ in inuse(path, "lock"):
        alive, _ = win["process_status"](pid)
        chain = list(win["process_chain"](pid, *snapshot))
        windows = win["matching_windows"]({owner for owner, _ in chain}, folder)
        first = next((depth for depth, (owner, _) in enumerate(chain) if owner in windows), None)
        emit(pid=pid, alive=alive, workspace_folder_known=bool(folder), chain=[
            {"pid": owner, "exe": exe, "depth": depth, "visible_window": owner in windows,
             "workspace_title_match": windows.get(owner, False) if folder else None,
             "first_visible": depth == first}
            for depth, (owner, exe) in enumerate(chain)])


class PrivateParser(argparse.ArgumentParser):
    def error(self, _message):
        emit(error="invalid_arguments")
        raise SystemExit(2)


def main():
    parser = PrivateParser()
    subs = parser.add_subparsers(dest="command", required=True)
    subs.add_parser("usage")
    subs.add_parser("done").add_argument("--summary", action="store_true")
    for command in ("procs", "hold"):
        subs.add_parser(command).add_argument("session_id")
    args = parser.parse_args()
    if args.command == "usage":
        usage_probe(sessions(30))
    elif args.command == "done":
        done_probe(sessions(), args.summary)
    elif not SESSION.fullmatch(args.session_id):
        parser.error("invalid session ID")
    elif sys.platform != "win32":
        emit(error="windows_required")
        return 2
    elif not (path := STATE / args.session_id).is_dir():
        emit(error="session_not_found")
        return 2
    else:
        process_probe(path, args.command)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, RecursionError) as exc:
        emit(error="probe_failed", windows_error=getattr(exc, "winerror", None))
        sys.exit(1)
