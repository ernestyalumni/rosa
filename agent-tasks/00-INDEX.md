# Agent Tasks — Index

Pick the lowest-numbered task whose **Blocked by** is satisfied.

| #  | Task                              | Owner role         | Blocked by | Brief                              |
|----|-----------------------------------|--------------------|------------|------------------------------------|
| 01 | LangChain rip-out + branch setup  | any                | —          | `01-langchain-ripout.md`           |
| 02 | Rust core scaffold (workspace)    | Rust agent         | —          | `02-rust-core-scaffold.md`         |
| 03 | LLM provider adapters             | Rust agent         | 02         | `03-rust-llm-adapters.md`          |
| 04 | Tool trait + registry             | Rust agent         | 02         | `04-rust-tool-trait.md`            |
| 05 | ROS 2 bridge (CLI then r2r)       | Rust + ROS         | 02         | `05-rust-ros2-bridge.md`           |
| 06 | Turtle demo port (Rust)           | Rust + ROS         | 03,04,05   | `06-turtle-demo-rust-port.md`      |
| 07 | Starship-on-Isaac integration     | Sim                | 06         | `07-starship-sim-config.md`        |

Out-of-repo briefs that block 06 and 07:

- `../../../Monoclaw/Deployments/ROS/AGENT_BRIEF.md` — blocks 06
- `../../../Monoclaw/Deployments/IsaacSim/AGENT_BRIEF.md` — blocks 07

## Conventions for sub-agents

- **Read** `../ORCHESTRATION.md` and `../ARCHITECTURE.md` before opening a brief — they pin the rules (no langchain, ROS 2 only, Rust-first, Docker only) that override anything ambiguous in a brief.
- **Branch** `feat/rosa-rust-<task-slug>` from `feat/rust-rewrite` (created in task 01). Never push to `main` or `master`.
- **Commits** in Conventional Commits style (`feat(rosa-core): ...`, `chore(deploy): ...`). One topic per PR.
- **Tests**: every public function in `rosa-core` / `rosa-tools` has a unit test. Adapters in `rosa-llm` use `wiremock` for HTTP-level tests.
- **No new langchain anywhere.** If you find yourself reaching for a Python helper, stop and ask in the PR description.
- **Mark this index** when your task lands: change the row to ✅ and link the merged PR.
