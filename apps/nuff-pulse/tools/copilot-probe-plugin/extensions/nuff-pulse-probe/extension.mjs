import { joinSession } from "@github/copilot-sdk/extension";
import { spawnSync } from "node:child_process";
import { appendFileSync, mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(homedir(), ".nuff-pulse-probe");
const uuid = /^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/i;
const keyPattern = /^[A-Za-z_][A-Za-z_0-9]{0,63}$/;
const exePattern = /^[A-Za-z0-9_. +()-]{1,100}$/;
const modelPattern = /^(?:auto|o[134]|hydrafusion|(?:claude|gpt|gemini|o[134]|mai|hydrafusion|copilot|text-embedding)-[A-Za-z0-9_.-]{1,78})$/;
const timePattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})$/;
const opaque = /^(?:args|arguments|toolArgs|input|content|text|prompt|title|response|reasoningText|reasoningSummary|requestMessages|last_assistant_message)$/i;
const dynamic = new Set(["agentMetrics", "modelMetrics", "models", "metadata", "headers", "env", "toolCounts"]);
const valueEvents = new Set(["session.usage_info", "assistant.usage", "session.title_changed",
    "session.idle", "assistant.turn_start"]);
const valueContainers = new Set(["usage", "tokenDetails", "promptTokensDetails", "completionTokensDetails",
    "copilotUsage", "contextWindow", "cache", "metrics", "tokenBreakdown", "accounting"]);

function append(name, record) {
    try {
        mkdirSync(root, { recursive: true, mode: 0o700 });
        appendFileSync(join(root, name), JSON.stringify(record) + "\n", { mode: 0o600 });
    } catch (error) {
        if (!error?.code) throw error;
        // Diagnostics must not affect the host session when the log is unavailable.
    }
}

function parentExes() {
    if (process.platform === "win32") {
        const helper = fileURLToPath(new URL("../../win_process.py", import.meta.url));
        const child = spawnSync("py", ["-3", "-I", helper, String(process.pid), String(process.ppid)],
            { encoding: "utf8", timeout: 1500, windowsHide: true, stdio: ["ignore", "pipe", "ignore"] });
        return child.status === 0 ? JSON.parse(child.stdout).slice(0, 8) : [];
    }
    const names = [];
    let pid = process.ppid;
    for (let i = 0; pid > 1 && i < 8; i++) {
        const child = spawnSync("ps", ["-p", String(pid), "-o", "ppid=", "-o", "comm="],
            { encoding: "utf8", timeout: 300, stdio: ["ignore", "pipe", "ignore"] });
        const row = child.status === 0 && /^(\d+)\s+(.+)$/.exec(child.stdout.trim());
        if (!row) break;
        pid = Number(row[1]);
        const exe = basename(row[2]);
        names.push(exePattern.test(exe) ? exe : "<exe>");
    }
    return names;
}

function schema(data) {
    const paths = new Map();
    function visit(value, path, key = "") {
        const type = value === null ? "null" : Array.isArray(value) ? "array" : typeof value;
        if (!paths.has(path)) paths.set(path, new Set());
        paths.get(path).add(type);
        if (opaque.test(key)) return;
        if (type === "array") {
            for (const item of value) visit(item, `${path}[]`, key);
        } else if (type === "object") {
            for (const [k, v] of Object.entries(value)) {
                const name = !dynamic.has(key) && keyPattern.test(k) ? k : "<key>";
                visit(v, `${path}.${name}`, k);
            }
        }
    }
    visit(data, "data");
    return Object.fromEntries([...paths].sort(([a], [b]) => a.localeCompare(b))
        .map(([path, types]) => [path, [...types].sort()]));
}

function safeValues(data, path = "data", out = {}) {
    for (const [key, value] of Object.entries(data ?? {})) {
        if (!keyPattern.test(key) || opaque.test(key)) continue;
        const field = `${path}.${key}`;
        if (typeof value === "number" && Number.isFinite(value)) {
            out[field] = value;
        } else if (typeof value === "string"
            && ((["model", "modelId", "currentModel"].includes(key) && modelPattern.test(value))
                || (key === "cacheExpiresAt" && timePattern.test(value)))) {
            out[field] = value;
        } else if (valueContainers.has(key) && value && typeof value === "object" && !Array.isArray(value)) {
            safeValues(value, field, out);
        }
    }
    return out;
}

const exe = basename(process.execPath);
const parents = parentExes();
append("ext-loaded.jsonl", {
    loaded: true, writeTimeMs: Date.now(), exe: exePattern.test(exe) ? exe : "<exe>", parentExes: parents,
});

const session = await joinSession();
if (typeof session.sessionId === "string" && uuid.test(session.sessionId)) {
    const name = `ext-${session.sessionId}.jsonl`;
    session.on((event) => {
        const type = /^[A-Za-z_][A-Za-z_0-9.]{0,100}$/.test(event.type) ? event.type : "<event>";
        const data = event.data ?? {};
        const record = {
            writeTimeMs: Date.now(), type,
            envelopeKeys: [...new Set(Object.keys(event).map((k) => keyPattern.test(k) ? k : "<key>"))].sort(),
            dataKeyPaths: schema(data),
            hasAgentId: Object.hasOwn(event, "agentId"),
            hasParentToolCallId: Object.hasOwn(data, "parentToolCallId"),
        };
        if (valueEvents.has(type)) record.values = safeValues(data);
        append(name, record);
    });
}
