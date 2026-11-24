# nuff-pulse (shared source, not a plugin)

Shared source of the `nuff-pulse-cc` and `nuff-pulse-copilot` hook plugins, which write the Pulse Files read by the nuff-pulse widget (`apps/nuff-pulse/`).

- `core/nuff_pulse_core.py` — Pulse Line building, process ancestry and the locked append. Change it only here, then copy it into `plugins/nuff-pulse-cc/hooks/` and `plugins/nuff-pulse-copilot/hooks/`.
- `tests/` — tests for the core, both adapters, the hook commands and that the copies match:

```powershell
py -3 -m unittest discover -s plugins/nuff-pulse/tests
```

The Pulse Line format is documented in each plugin's README; keep both in step with the core.
