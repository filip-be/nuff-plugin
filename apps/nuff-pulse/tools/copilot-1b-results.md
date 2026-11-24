# Copilot probe 1B

## Per-request usage on disk

- (V) `usage` scanned the 30 most recently modified session event logs. `session.usage_record` had numeric `data.usage.inputTokens`, `data.usage.cacheReadTokens`, and `data.usage.cacheWriteTokens` in 279/279 records. `cacheDetailsReported` was true in 279/279; cache reads were nonzero in 249 and cache writes in 205.
- (V) `model.model_call_success` had numeric `data.copilotUsage.token_details[].token_count` for `token_type` values `input`, `cache_read`, and `cache_write` in 44/44 records. All 44 also had numeric `data.responseChunk.usage.prompt_tokens` and `data.responseChunk.usage.prompt_tokens_details.{cached_tokens,cache_write_tokens,cache_creation_tokens}`. These are alternative views of request usage, not additional tokens to add together.
- (V) The sample had 42 `session.shutdown` records with `data.currentTokens`, and 0 `session.compaction_start`, `session.compaction_complete`, or `compaction_complete` records. The probe paired main-thread requests with the nearest context snapshot in the same session start/resume interval. Deltas are request figure minus `currentTokens`:

| Request figure | Pairs | Exact matches | Median delta, tokens | Median absolute time gap, ms |
| --- | ---: | ---: | ---: | ---: |
| `session.usage_record`: input | 248 | 0 | -46,887 | 970,355 |
| `session.usage_record`: input + cache read + cache write | 248 | 0 | 19,133 | 970,355 |
| `model.model_call_success`: input | 10 | 0 | -189,893 | 959,047.5 |
| `model.model_call_success`: input + cache read + cache write | 10 | 0 | -182,703 | 959,047.5 |
| `model.model_call_success`: `prompt_tokens` | 10 | 0 | -182,703 | 959,047.5 |

Use these events for per-request usage, not as a verified current-context counter. The shutdown comparisons did not agree, but they are not simultaneous measurements; the sample cannot establish how closely request usage tracks context at request time or around compaction.

- (V) `session.usage_checkpoint` had `data.modelCacheState` (array) in 47/47 records. In 36/47 it contained objects with `data.modelCacheState[].modelId` (string), `data.modelCacheState[].cacheExpiresAt` (string), and `data.modelCacheState[].cacheTtlSeconds` (integer). The probe also prints the recursive schema for the other checkpoint fields without their values.

## Done signal on disk

### Initial baseline


- (V) `done` scanned 218 session event logs and grouped 335 main-thread turns. Steering delivered during an existing turn was kept in that reply. `disk continued` means more assistant/model activity or a new blocker appeared in the same reply after the candidate. It is disk evidence of an early candidate, not a live `Done` label.

| Candidate | Events | Disk continued | Turns without a candidate |
| --- | ---: | ---: | ---: |
| Last eligible `assistant.turn_end` per turn | 280 | 11 | 55 |
| Every eligible `assistant.turn_end` as seen live | 284 | 15 | 55 |
| Every `model.turn_ended` | 1,400 | 1,388 | 262 |

- (V) Probe `agentStop` and `session.idle` reference turns: 0 each. Reference offsets and missed Done are unmeasured, not zero. A turn without a disk candidate is not necessarily a missed Done: it may still be running, have aborted, or lack a complete trace.
- `assistant.turn_end` with a last message that requests no tools and no open blockers is the better disk candidate in this sample, but even retrospectively selecting the last eligible event produced early candidates. `model.turn_ended` is not suitable on its own.

### Three-host rerun (2026-11-24)

- (V) Ran `probe_copilot.py done` after `agentStop` records existed; neither extension log existed. The corpus contained 224 sessions and 346 turns. The probe matched 8 `agentStop` reference turns and 0 `session.idle` reference turns. These are corpus-wide counts, not just the three test turns.
- (V) `assistant.turn_end` covered all 8 reference turns. Across the corpus the online filter found 294 candidates, 15 of which were followed by more work on disk. `model.turn_ended` had 1,420 candidates, 1,408 followed by more work, and missed all 8 `agentStop` reference turns.
- (V) On the completed app, VS Code, and Terminal test turns, the final `assistant.turn_end` preceded the `agentStop` log write by 839 ms, 1,145 ms, and 1,147 ms respectively; none of those final candidates was followed by more work in the same turn. This measures hook write time, not when the answer first became visible. No live `session.idle` behavior was verified.

## Extension and hooks by host

| Host and session-state ID | `workspace.yaml` `client_name` | Hook parent chain (nearest first) | `ext-loaded.jsonl` / `ext-<id>.jsonl` | `hooks-<id>.jsonl` and observed hooks |
|---|---|---|---|---|
| (V) Copilot app, current session `d282a607-afd7-49c8-8619-25da9acf6167` | `github` | `py.exe` > `pwsh.exe` > `copilot-runtime.exe` > `github.exe` | No / No | Yes; `sessionStart` at the snapshot, with this turn still running |
| (V) VS Code Agent Host `b6b633ca-84b3-4447-8c4b-9c217c933798` | `vscode-agent-host` | `py.exe` > `pwsh.exe` > `copilot-runtime.exe` > `Code.exe` > `Code.exe` > `explorer.exe` | No / No | Yes; one each of `sessionStart`, `agentStop`, and `sessionEnd` (on window close) |
| (V) Copilot CLI in Windows Terminal `08bb40d5-110c-4ba2-a302-3277518a075a` | `github` | `py.exe` > `pwsh.exe` > `copilot.exe` > `pwsh.exe` > `WindowsTerminal.exe` > `explorer.exe` | No / No | Yes; one each of `sessionStart`, `agentStop`, and `sessionEnd` (after `/exit`) |

- (V) The previous completed app test session, `3b8315f8-4bd2-4422-9400-59e6e5cd67b7`, also had `client_name: github` and the same app chain and runtime PID as the current session. Its hook log contained `sessionStart` and `agentStop`, with no `sessionEnd` observed; both extension logs were absent.
- (V) The installed probe package contained the extension entry point, but the shared load log was absent throughout the run. It could not corroborate host identity or establish why the extension produced no output. `client_name: github` alone did not distinguish the app from the CLI.

## Subagent usage attribution

| Host | `subagent.started` to `subagent.completed` in `events.jsonl` (UTC) | `assistant.usage` / `session.usage_info` on disk | Live extension usage |
|---|---|---|---|
| (V) Copilot app, current and previous completed test | No start or completion marker in either sampled session | 0 / 0 in both | No file |
| (V) VS Code Agent Host | 1 pair: `09:52:38.598` to `09:53:05.779` (27.181 s) | 0 / 0, including inside the window | No file |
| (V) Terminal CLI | 1 pair: `09:53:30.997` to `09:53:48.938` (17.941 s) | 0 / 0, including inside the window | No file |

- (V) Both recorded start/completion pairs carried an `agentId`; neither pair carried `data.parentToolCallId`. Those are lifecycle markers, not usage events. There were no captured extension rows to count or align with either window, and no evidence either way about `agentId` or `data.parentToolCallId` on live usage events.
- **Main-thread usage cannot be distinguished from subagent usage with these captures.** That does not prove the live events lack attribution. Do not infer ownership from timing alone or infer that the app never ran a subagent from the absence of a marker.

## Focus mapping on Windows

| Host | `procs` chain from lock owner | Is the first visible-window process the right target? |
|---|---|---|
| (V) Copilot app (both sessions) | `copilot-runtime.exe` > `github.exe` | Yes for the app window. The two sampled sessions shared this process chain; it does not identify an individual chat. |
| (V) VS Code Agent Host | `copilot-runtime.exe` > `Code.exe` > `Code.exe` > `explorer.exe` | Yes for the tested VS Code window: the second `Code.exe` was first visible, matched the session's recorded workspace folder, and exited when the user closed that window. The recorded folder was not the current checkout root. |
| (V) Terminal CLI | `copilot.exe` > `pwsh.exe` > `WindowsTerminal.exe` > `explorer.exe` | No. `pwsh.exe` was first visible; `WindowsTerminal.exe` was the next ancestor and also visible. This identifies the Terminal window process, not a tab. |

## .hold liveness

| Check | Lock PID alive? | Exclusive open of `.hold` | `.lock` / `.hold` left on disk? |
|---|---|---|---|
| (V) Copilot app live; rechecked after the other hosts closed | Yes, `49592` | No; Windows error 32 (sharing violation) | Present / present |
| (V) Original VS Code Agent Host live | Yes, `47420` | No; error 32 | Present / present |
| (V) Original Terminal CLI live | Yes, `31944` | No; error 32 | Present / present |
| (V) Original Terminal CLI after `/exit` | No, `31944` exited | No file to open; `hold` emitted no row | Absent / absent |
| (V) VS Code after closing the test window | No, `47420` exited | No file to open; `hold` emitted no row | Absent / absent |
| (V) Fresh Terminal CLI `6b9cd7bf-f058-4740-a90b-7bf5007ae573` before closing the tab | Yes, `23616` | No; error 32 | Present / present |
| (V) Fresh CLI after closing the Terminal tab without `/exit` | No, `23616` exited | No file to open; `hold` emitted no row | Absent / absent |

- (V) Closing the Terminal tab still fired `sessionEnd` and cleaned up both files. This tested tab closure, not an abrupt process kill: neither leftover openable files nor hard-kill cleanup were verified. The app was intentionally left open; its shutdown was not tested. No CIM/WMI was used.

## Probe cleanup

Close any remaining Copilot hosts using the probe so they cannot recreate the logs. From PowerShell, uninstall the registered plugin, then delete its local output directory:

```powershell
copilot plugin uninstall nuff-pulse-probe
```

```powershell
Remove-Item -LiteralPath (Join-Path $HOME '.nuff-pulse-probe') -Recurse -Force
```

## Plan assumptions and widget behavior

- **Holds:** All three hosts wrote lifecycle hooks. `agentStop` was a usable Done signal on the sampled completed turns. All live samples had a sharing violation and a live lock PID; the three tested close paths removed both files.
- **Doesn't hold or isn't verified:** The extension did not provide a live stream on any sampled host. Disk turn-end events alone are not reliable Done signals, usage attribution is unknown, and the first visible ancestor is not always the right focus target. Tab close did not exercise stale-lock behavior after a hard kill.
- **Widget:** Prefer a verified `agentStop` signal for Done; otherwise treat disk turn-end events as provisional and supersede them on later activity. Use session-level usage only where available and label ownership unknown rather than guessing. Resolve focus by host family (skip the shell for Terminal), and do not promise tab or chat selection. Combine exclusive-open results with validated process identity; distinguish missing files from sharing violations, stale or unavailable state, and other open errors.
