# nuff-hooks (shared source)

Shared source of the guardrail hooks shipped by [nuff-hooks-cc](../nuff-hooks-cc/README.md) (Claude Code) and [nuff-hooks-copilot](../nuff-hooks-copilot/README.md) (GitHub Copilot CLI). This folder is **not a plugin** and is not listed in the marketplace.

- [Glossary](../../CONTEXT.md#guardrail-hooks) — Workspace, Read Roots, Read Zone, Write Zone, Trash, Removal, Rewrite (in the root CONTEXT.md)
- Read Roots are configured in `~/.nuff-hooks.json` — see [nuff-hooks-cc](../nuff-hooks-cc/README.md#read-roots)
- [core/nuff_hooks_core.py](./core/nuff_hooks_core.py) — platform-neutral checks; the canonical copy
- [tests/](./tests) — unit tests, the copy-sync test and tests of the `hooks.json` launch commands (`py`, else `python3`, else deny)

## Changing the core

Edit `core/nuff_hooks_core.py`, then copy it into both plugins and run the tests:

```bash
cp plugins/nuff-hooks/core/nuff_hooks_core.py plugins/nuff-hooks-cc/hooks/
cp plugins/nuff-hooks/core/nuff_hooks_core.py plugins/nuff-hooks-copilot/hooks/
py -3 -m unittest discover -s plugins/nuff-hooks/tests -v   # python3 on Linux and macOS
```

The tests check both the Windows and the Linux/macOS rules on any OS (they switch `WINDOWS` in the core).

Bump the version of both plugins when the core changes.
