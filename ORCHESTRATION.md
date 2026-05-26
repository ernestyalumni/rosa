# ROSA Fork — Orchestration Plan

**Owner:** Ernest Yeung (`ernestyalumni/rosa` fork of `nasa-jpl/rosa`)
**Started:** 2026-05-24
**Motivating context:** Matter Intelligence FSW interview prep (Tue 2026-05-26) + portfolio demo of a Rust-first agentic robotics stack. See `Galvatron/Documents/MatterIntelligence/ROSA_PROJECT_CONTEXT.md` for the interview tie-back.

---

## North Star

Replace upstream ROSA (Python + LangChain + ROS1/ROS2 wrapper) with a **Rust-first agent** that talks to **ROS 2 only**, runs in **Docker**, exercises a **turtlesim** sanity check, then commands a **Starship-stand-in robot in NVIDIA Isaac Sim**.

Hard rules:

1. **Zero LangChain.** Any line that imports `langchain*` gets deleted or replaced.
2. **Rust by default.** Python only as an explicit escape hatch where a Rust ROS 2 binding has a real gap, and only behind a feature flag.
3. **ROS 2 only.** ROS 1 Noetic EOL'd April 2025; we drop `src/rosa/tools/ros1.py` and the ROS 1 turtle demo. We re-implement the turtle demo on ROS 2 turtlesim.
4. **No host installs.** ROS 2 and Isaac Sim live in Docker. Anything that touches the host is suspect.
5. **Inspired by, not coupled to.** `hermes-agent` and `openclaw` are reference implementations — we read them, take ideas, write our own Rust. We do NOT add them as runtime dependencies.

---

## Architecture in one diagram

```
┌────────────────────────────────────────────────────────────────────┐
│  Host                                                              │
│                                                                    │
│  ┌─────────────────────┐    DDS    ┌───────────────────────────┐   │
│  │  rosa-cli (Rust)    │◀─────────▶│  ros2-stack (Docker)      │   │
│  │  - agent loop       │   topic   │  - osrf/ros:humble        │   │
│  │  - llm adapters     │   svc     │  - turtlesim node         │   │
│  │  - tool registry    │   param   │  - rosbridge_server (opt) │   │
│  │  - r2r ROS2 client  │           └───────────────────────────┘   │
│  └─────────┬───────────┘                          ▲                │
│            │                                      │ ROS 2 bridge   │
│            │                                      │ extension      │
│            │                              ┌───────┴───────────────┐│
│            │                              │ isaac-stack (Docker)  ││
│            └────── HTTPS ───── LLM        │  - nvcr.io/.../isaac  ││
│                                provider   │  - Starship stand-in  ││
│                                (OpenAI/   │  - GPU: RTX 3060 12G  ││
│                                Anthropic/ │    or RTX 3070 8G     ││
│                                Ollama)    └───────────────────────┘│
└────────────────────────────────────────────────────────────────────┘
```

See `ARCHITECTURE.md` for crate layout, message contracts, and the tool trait.

---

## Phases (sequence + parallelism)

| # | Phase                       | Owner        | Blocking | Parallel with | Status |
|---|-----------------------------|--------------|----------|---------------|--------|
| 1 | LangChain rip-out + branch  | any agent    | —        | 2             | ✅ done — commit 9e16804 |
| 2 | Rust core scaffold          | Rust agent   | —        | 1             | ✅ done — commit 6f7679c; 5/5 tests pass |
| 3 | LLM provider adapters       | Rust agent   | 2        | 4, 5          | ⏳ ready to start |
| 4 | Tool trait + registry       | Rust agent   | 2        | 3, 5          | ⏳ ready to start |
| 5 | ROS 2 bridge (r2r/rclrs)    | Rust agent   | 2        | 3, 4          | ⏳ ready to start |
| 6 | Turtle demo port (Rust)     | Rust agent   | 3,4,5    | Monoclaw-ROS  | ⏳ blocked on 3+4+5 |
| 7 | Isaac + Starship sim        | Sim agent    | 6        | —             | ⏳ blocked on 6 |

Monoclaw Docker tracks:
- `Monoclaw/Deployments/Stacks/ROS/` — ✅ done — langchain stripped, host-network DDS, Rust toolchain. Branch: `feat/ros2-deploy-strip-langchain`.
- `Monoclaw/Deployments/Stacks/IsaacSim/` — ✅ done (first pass) — Dockerfile, compose, scripts created. USD + extensions pending task 07.

Two parallel Docker tracks live in Monoclaw, not this repo:

- `Monoclaw/Deployments/Stacks/ROS/AGENT_BRIEF.md` — drop the langchain pip install, keep ROS 2 base + Rust toolchain. Blocks phase 6.
- `Monoclaw/Deployments/Stacks/IsaacSim/AGENT_BRIEF.md` — net-new. RTX 3060 12 GB confirmed adequate. Blocks phase 7.

Per-phase briefs live in `agent-tasks/`. Each is self-contained so a fresh Codex/Claude/openclaw session can pick up cold.

---

## How a sub-agent picks up work

1. Read `agent-tasks/00-INDEX.md` to find an unblocked task.
2. Read that task's `agent-tasks/NN-*.md` brief end-to-end — it lists files to read, files to write, acceptance criteria, and **why** the choice was made (so you can deviate sensibly if the premise is wrong).
3. Work on a feature branch (`feat/rosa-rust-<task-slug>`). Never push to `main` — the fork's `main` tracks `upstream/main` from JPL.
4. Open a PR against the fork's own `dev`-equivalent branch (we'll set this up as `feat/rust-rewrite` integration branch — see phase 1 brief).
5. Update this file's phase table when your task lands.

---

## Files this orchestration owns

```
rosa/
├── ORCHESTRATION.md          ← you are here
├── ARCHITECTURE.md           ← crate layout, traits, message contracts
└── agent-tasks/
    ├── 00-INDEX.md           ← pick-a-task entry point
    ├── 01-langchain-ripout.md
    ├── 02-rust-core-scaffold.md
    ├── 03-rust-llm-adapters.md
    ├── 04-rust-tool-trait.md
    ├── 05-rust-ros2-bridge.md
    ├── 06-turtle-demo-rust-port.md
    └── 07-starship-sim-config.md
```

Cross-repo briefs:

```
Monoclaw/Deployments/
├── ROS/AGENT_BRIEF.md        ← strip langchain, keep ROS 2 + Rust
└── IsaacSim/AGENT_BRIEF.md   ← net-new Docker stack
```

```
Galvatron/Documents/MatterIntelligence/
└── ROSA_PROJECT_CONTEXT.md   ← why this project, interview tie-back
```
