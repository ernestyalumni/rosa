# Task 06 — Turtle Demo Port (Rust)

**Owner role:** Rust + ROS agent
**Blocked by:** 03, 04, 05 (5a is enough; 5b helps but not required), plus `Monoclaw/Deployments/ROS/AGENT_BRIEF.md`
**Blocks:** 07
**Estimate:** 3–5 hr

## Goal

Re-implement upstream rosa's TurtleSim demo as `examples/turtle/` running entirely on Rust + ROS 2 (no ROS 1, no Python). User asks rosa-cli to draw a 5-point star, rosa picks the right tools, turtlesim window shows the star.

This is the **sanity-check end-to-end test** of the whole rewrite. If this works, the architecture is validated.

## Context — what we're matching

Upstream demo (now deleted from `src/turtle_agent/` per task 01) used ROS 1 + langchain + rospy. It registered `cool_turtle_tool`, `blast_off`, plus a `turtle_tools` package with movement helpers. The agent could draw shapes by reasoning about coordinates → `Twist` publishes.

Behavioral spec: at the REPL prompt, the user types `"Draw a 5-point star using the turtle."` and rosa:
1. Calls `ros2_list_topics` to find `/turtle1/cmd_vel`.
2. Optionally calls `turtle_get_pose`.
3. Issues a sequence of `turtle_publish_twist` calls (with linear/angular velocity) to trace the star.
4. Returns a "done" message.

## Files to create

```
rosa/
├── crates/
│   └── rosa-cli/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs           # binary: `rosa`
│           ├── repl.rs           # rustyline loop
│           ├── render.rs         # AgentEvent → crossterm output
│           └── config.rs         # ~/.config/rosa/config.toml
└── examples/turtle/
    ├── README.md                 # how to run the demo
    ├── run.sh                    # convenience wrapper
    ├── system_prompt.md          # turtle-specific system prompt
    └── src/main.rs               # `rosa-turtle` example binary
```

`examples/turtle/src/main.rs` registers:
- All `rosa-ros2` tools from task 05.
- Turtle-specific tools:
  - `turtle_get_pose(name: String) -> Pose` (via `ros2_subscribe_once` on `/{name}/pose`)
  - `turtle_set_pen(name, r, g, b, width, off)` (via `ros2_call_service` on `/{name}/set_pen`)
  - `turtle_teleport_absolute(name, x, y, theta)` (via `/{name}/teleport_absolute` service)
  - `turtle_publish_twist(name, linear, angular, duration_s)` (high-level: publishes Twist for duration_s then stops)
  - `turtle_clear()` (service call `/clear`)
  - `turtle_within_bounds(x, y)` (pure-Rust, no ROS)

Load `system_prompt.md` content into the agent's system prompt at startup.

## How to run

```bash
# 1. Start ROS 2 + turtlesim container
cd /home/propdev/.openclaw/workspace/repos/Monoclaw/Deployments/ROS
docker compose up -d
docker compose exec ros2 ros2 run turtlesim turtlesim_node &  # GUI via X11

# 2. From the rosa repo (with OPENAI_API_KEY or ANTHROPIC_API_KEY set)
cd /home/propdev/.openclaw/workspace/workspace2/repos/rosa
cargo run --release --example turtle

# 3. At the prompt
> Draw a 5-point star using the turtle.
```

## Acceptance criteria

- [ ] `cargo run --example turtle` builds and starts the REPL with the turtle-specific system prompt loaded.
- [ ] Against a running `Monoclaw/Deployments/ROS` stack with turtlesim running, the prompt `"Move turtle1 forward 2 units"` produces a visible motion.
- [ ] The prompt `"Draw a 5-point star using the turtle"` produces a star (recorded as a screenshot in the PR).
- [ ] Streaming output renders tokens incrementally in the terminal.
- [ ] `Ctrl-C` mid-response cancels cleanly (matches upstream's `GracefulInterruptHandler`).
- [ ] Token usage tracking (if `OPENAI_API_KEY` provider) printed at end of turn.

## Out of scope

- Multi-turtle scenarios (defer).
- Vision / image input (defer).
- Anything Isaac Sim — that's task 07.

## Demo recording for the interview

Save a screen-cap (asciinema or mp4) of the 5-point star session into `examples/turtle/demo.{cast,mp4}` — this is the artifact that goes into the Matter Intelligence portfolio link.
