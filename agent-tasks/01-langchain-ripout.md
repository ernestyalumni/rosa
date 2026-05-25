# Task 01 — LangChain Rip-out & Branch Setup

**Owner role:** any agent (mechanical edit, no architecture decisions)
**Blocked by:** —
**Blocks:** 02–07 (everything downstream assumes a clean branch)
**Estimate:** 30–60 min

## Goal

Stand up the integration branch `feat/rust-rewrite` and physically delete every line in the repo that imports or references `langchain`. The repo must `grep -ri langchain` to zero matches in source (matches in `CHANGELOG.md` historical entries are OK if quoted as removed).

## Context — why

Rosa's runtime today is ~90% LangChain glue (see `src/rosa/rosa.py` lines 21–27, 256–276). Keeping it imported "just in case" will pull it back into any new module a sub-agent writes by reflex. Delete first; rebuild on Rust (see tasks 02–05).

## Steps

1. **Branch**:
   ```bash
   cd /home/propdev/.openclaw/workspace/workspace2/repos/rosa
   git fetch upstream
   git checkout -b feat/rust-rewrite main
   ```
2. **Inventory** every langchain reference:
   ```bash
   grep -rn 'langchain' --include='*.py' --include='*.toml' --include='*.md' --include='Dockerfile*' .
   ```
   Expected hits: `src/rosa/rosa.py`, `src/rosa/tools/__init__.py`, `src/rosa/tools/ros{1,2}.py`, `src/rosa/tools/system.py`, `src/rosa/tools/log.py`, `src/rosa/tools/calculation.py`, `src/rosa/prompts.py`, `src/turtle_agent/scripts/{turtle_agent,llm,tools/turtle}.py`, `pyproject.toml`, `Dockerfile`, `README.md`.
3. **Delete** the Python implementation directories outright:
   - `rm -rf src/rosa/` (whole package — will be replaced by Rust crates in task 02)
   - `rm -rf src/turtle_agent/scripts/` (replaced in task 06)
   - Keep `src/turtle_agent/{CMakeLists.txt, package.xml, launch/}` — these are ROS package metadata we'll repurpose.
4. **Strip** `pyproject.toml`:
   - Remove the `[project]`, `[project.optional-dependencies]`, `[tool.setuptools.packages.find]` blocks entirely.
   - Replace top of file with a comment: `# Python packaging removed in feat/rust-rewrite. See ARCHITECTURE.md.`
   - Delete `setup.py` (`rm setup.py`).
5. **Strip** the root `Dockerfile`:
   - Replace it with a 1-line stub: `# Deprecated. See Monoclaw/Deployments/ROS/Dockerfile and rosa-cli's Dockerfile.` (or delete and reference Monoclaw).
6. **Strip** `demo.sh`: delete (replaced by `examples/turtle/run.sh` in task 06).
7. **README.md**: replace the "Quick Start" Python `pip install jpl-rosa` block with a "🚧 Under Rewrite — see ORCHESTRATION.md" note. Leave the upstream credit/links intact.
8. **Re-grep** to confirm zero hits:
   ```bash
   grep -ri 'langchain' --include='*.py' --include='*.rs' --include='*.toml' --include='Dockerfile*' . || echo OK
   ```

## Acceptance criteria

- [ ] Branch `feat/rust-rewrite` exists locally and pushed to `origin`.
- [ ] `grep -ri 'langchain' --include='*.py' --include='*.rs' --include='*.toml' --include='Dockerfile*' .` returns no hits (CHANGELOG/historical markdown OK).
- [ ] `git status` clean after commit.
- [ ] PR opened against `feat/rust-rewrite` (or merged directly if you're the first onto the branch); commit message: `chore: remove LangChain runtime ahead of Rust rewrite`.

## Out of scope

- Do not start adding Rust code in this PR. Task 02 owns the workspace scaffold.
- Do not touch `LICENSE`, `CHANGELOG.md`, `GOVERNANCE.md`, `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`, `SECURITY.md` — upstream metadata stays.

## Verification

```bash
cd /home/propdev/.openclaw/workspace/workspace2/repos/rosa
git checkout feat/rust-rewrite
grep -rn 'langchain' src/ pyproject.toml Dockerfile 2>/dev/null  # → no output
ls src/  # → should only contain turtle_agent/{CMakeLists.txt, package.xml, launch/}
```
