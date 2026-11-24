"""Write sanitized Copilot session fixtures for the nuff-pulse widget tests.

Usage: py -3 -I tools/fixtures_copilot.py OUT_DIR NAME=SESSION_ID [NAME=SESSION_ID ...]

Reads `$COPILOT_HOME/session-state/<id>/events.jsonl` and `workspace.yaml`
(default `~/.copilot`) and writes `OUT_DIR/<NAME>/events.jsonl` and
`OUT_DIR/<NAME>/workspace.yaml`:

- enum-like values (event types, tool names, models, modes) are kept;
- ids are remapped consistently per session; the session id becomes a fixed
  fake UUID;
- timestamps are shifted so each session starts at 2025-11-24T16:00:00Z;
- paths become C:\\work\\project, repositories owner/project, branches feature;
- every other string becomes `<str:N>` (N = original length);
- bulky event types (hooks, model traces, system prompts) and bulky data keys
  are dropped; numbers are kept.
"""

import json
import os
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

BASE = 1764000000.0
KEEP = {
    "type", "toolName", "toolTitle", "model", "newModel", "previousModel",
    "selectedModel", "currentModel", "modelId", "responseModel", "phase",
    "agentMode", "permissionMode", "producer", "copilotVersion", "errorType",
    "infoType", "reason", "shutdownType", "decisionSource", "agentName",
    "agentType", "executionMode", "actor", "kind", "hostType", "delivery",
    "turnId", "stopReason", "reasoningEffort", "modelDisplayName", "initiator",
    "responseKind", "repositoryHost", "taskModelSource", "modelSelectionSource",
}
TOOL_NAME_SUFFIX = ("toolRequests", "[]", "name")
IDS = {
    "id", "parentId", "agentId", "toolCallId", "requestId", "interactionId",
    "messageId", "originatingMessageId", "parentToolCallId", "ownerSessionId",
    "ownerNativeSessionId", "parentAgentTaskId", "agentTaskId", "apiCallId",
    "usageId", "sourceSessionId", "providerCallId", "serviceRequestId",
}
TIMES = {"timestamp", "startTime", "resumeTime", "cacheExpiresAt"}
DROP_TYPES = ("hook.", "system.", "instruction.", "mcp.", "model.")
KEEP_MODEL_TYPES = {"model.turn_ended"}
DROP_KEYS = {
    "accountingSnapshot", "providerModelMetrics", "usageAccountingWatermarks",
    "modelMetrics", "promptCacheBreakState", "toolTelemetry", "result",
    "shellExecution", "rte", "copilotUsage", "shellToolInfo", "arguments",
    "reasoningText", "reasoningOpaque", "encryptedContent", "reasoningBlocks",
    "transformedContent", "supportedNativeDocumentMimeTypes", "serverTools",
    "permissionRequest", "promptRequest", "assistedApprovalAssessment",
    "githubMcpToolConfig", "agentMetrics", "codeChanges", "tokenDetails",
    "causalDependencies", "attachments", "tools", "agentDescription",
    "responsesReasoning", "fileEdits", "modelInfo", "headCommit", "response",
    "quotaSnapshots",
}
ISO = re.compile(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d+)?Z$")


def parse_ts(v):
    try:
        return datetime.fromisoformat(v.replace("Z", "+00:00")).timestamp()
    except ValueError:
        return None


def fmt_ts(t):
    s = datetime.fromtimestamp(t, timezone.utc).isoformat(timespec="milliseconds")
    return s.replace("+00:00", "Z")


class Sanitizer:
    def __init__(self, session_id, fake_session_id, t0):
        self.ids = {session_id: fake_session_id}
        self.session_id = session_id
        self.fake_session_id = fake_session_id
        self.t0 = t0

    def fake_id(self, v):
        if v not in self.ids:
            self.ids[v] = f"id-{len(self.ids):04d}"
        return self.ids[v]

    def time(self, v):
        t = parse_ts(v)
        return v if t is None else fmt_ts(BASE + (t - self.t0))

    def clean(self, v, path=()):
        if isinstance(v, dict):
            return {k: self.clean(x, path + (k,)) for k, x in v.items() if k not in DROP_KEYS}
        if isinstance(v, list):
            return [self.clean(x, path + ("[]",)) for x in v]
        if not isinstance(v, str) or not path:
            return v
        key = path[-1]
        if key in KEEP or path[-3:] == TOOL_NAME_SUFFIX:
            return v
        if key == "sessionId":
            return self.fake_id(v)
        if key in IDS:
            return self.fake_id(v)
        if key in TIMES and ISO.match(v):
            return self.time(v)
        if key in ("cwd", "gitRoot"):
            return "C:\\work\\project"
        if key == "repository":
            return "owner/project"
        if key == "branch":
            return "feature"
        if key == "source":
            if v.startswith("agent-"):
                return "agent-" + self.fake_id(v[len("agent-"):])
            return v
        return f"<str:{len(v)}>"


def keep_event(rec):
    t = rec.get("type", "")
    return t in KEEP_MODEL_TYPES or not t.startswith(DROP_TYPES)


def clean_workspace(text, s):
    out = []
    user_named = "user_named: true" in text
    for line in text.splitlines():
        key, sep, value = line.partition(": ")
        if not sep:
            out.append(line)
            continue
        if key == "id":
            value = s.fake_session_id
        elif key in ("cwd", "git_root"):
            value = "C:\\work\\project"
        elif key == "repository":
            value = "owner/project"
        elif key == "branch":
            value = "feature"
        elif key == "name" and not user_named:
            value = "Test prompt session"
        elif key in ("created_at", "updated_at"):
            value = s.time(value)
        out.append(f"{key}: {value}")
    return "\n".join(out) + "\n"


def main(argv):
    if len(argv) < 3:
        print(__doc__)
        return 2
    out = Path(argv[1])
    home = Path(os.environ.get("COPILOT_HOME") or Path.home() / ".copilot")
    for n, arg in enumerate(argv[2:], 1):
        name, _, sid = arg.partition("=")
        src = home / "session-state" / sid
        records = []
        with open(src / "events.jsonl", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line:
                    try:
                        records.append(json.loads(line))
                    except json.JSONDecodeError:
                        pass
        t0 = parse_ts(records[0]["timestamp"])
        fake = f"00000000-0000-4000-8000-{n:012d}"
        s = Sanitizer(sid, fake, t0)
        dest = out / name
        dest.mkdir(parents=True, exist_ok=True)
        with open(dest / "events.jsonl", "w", encoding="utf-8", newline="\n") as f:
            for rec in records:
                if keep_event(rec):
                    f.write(json.dumps(s.clean(rec), ensure_ascii=False, separators=(",", ":")) + "\n")
        ws = (src / "workspace.yaml").read_text(encoding="utf-8")
        (dest / "workspace.yaml").write_text(clean_workspace(ws, s), encoding="utf-8", newline="\n")
        print(f"{name}: {sid} -> {fake}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
