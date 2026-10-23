# nuff-hooks (shared source)

Shared source of the guardrail hooks shipped by [nuff-hooks-cc](../nuff-hooks-cc/README.md) (Claude Code) and [nuff-hooks-copilot](../nuff-hooks-copilot/README.md) (GitHub Copilot CLI). This folder is **not a plugin** and is not listed in the marketplace.

- [Glossary](../../CONTEXT.md#guardrail-hooks) — Workspace, Read Zone, Write Zone, Trash, Removal, Rewrite (in the root CONTEXT.md)
- [core/nuff_hooks_core.py](./core/nuff_hooks_core.py) — platform-neutral checks; the canonical copy
- [tests/](./tests) — unit tests and the copy-sync test

## Changing the core

Edit `core/nuff_hooks_core.py`, then copy it into both plugins and run the tests:

```bash
cp plugins/nuff-hooks/core/nuff_hooks_core.py plugins/nuff-hooks-cc/hooks/
cp plugins/nuff-hooks/core/nuff_hooks_core.py plugins/nuff-hooks-copilot/hooks/
py -3 -m unittest discover -s plugins/nuff-hooks/tests -v
```

Bump the version of both plugins when the core changes.
