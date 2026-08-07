---
name: challenge-domain-model
description: Challenge existing domain model, CLAUDE.md/AGENTS.md and README files structure
disable-model-invocation: true
---

# Challenge domain model

Challenge the existing artifacts and propose a better structure for the repository documentation.

Follow these steps:
1. Search and read existing files:
  - `CONTEXT.md` and `CONTEXT-MAP.md` files
  - `CLAUDE.md` and `AGENTS.md` files
  - `README.md` files
  - `docs/adr/` folders
  - `.github/copilot-instructions.md` file
2. Run a `/grilling` session, using the `/domain-modeling` skill to challenge the existing domain model, CLAUDE.md/AGENTS.md and README files structure.

## Known issues to look for:

Below list is not exhaustive:
- `copilot-instructions.md` file is present. Suggest to split its content into domain-specific `CLAUDE.md` or `AGENTS.md` files
- `CLAUDE.md` exists together with `AGENTS.md`. Suggest to use CLAUDE.md only - GitHub Copilot also supports CLAUDE.md files.
- `CONTEXT.md` file is missing. Suggest to create one and move the glossary here. Mind that this may need to run several sub-agents to explore the codebase and build the glossary.
- `README.md`/`CLAUDE.md`/`AGENTS.md` files are too long. Suggest to remove some faff from them and move some content around.
