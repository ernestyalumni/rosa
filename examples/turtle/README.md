# rosa Turtle Demo

End-to-end demo: rosa (Rust) controlling TurtleSim via ROS 2.

This is the **sanity-check** for the entire rosa Rust rewrite. If the turtle
draws a 5-point star, all layers are working:
- `rosa-core` agent loop (LLM streaming + tool fan-out)
- `rosa-llm` Anthropic/OpenAI adapters (SSE + tool_use parsing)
- `rosa-tools` ToolRegistry dispatch
- `rosa-ros2` CLI shell-out tools (ros2 node/topic/service wrappers)
- `rosa-cli` turtle-specific tools (Twist publish, pose read, services)

---

## Prerequisites

### 1. ROS 2 container running
```bash
cd /home/propdev/.openclaw/workspace/repos/Monoclaw/Deployments/Stacks/ROS
docker compose up -d
```

### 2. TurtleSim node running (needs X11)
```bash
docker compose exec ros2 bash -ic "ros2 run turtlesim turtlesim_node &"
```

Or open two terminals:
- Terminal A: `docker compose exec ros2 bash -ic "ros2 run turtlesim turtlesim_node"`
- Terminal B: the rosa REPL (below)

### 3. API key
At least one of:
- `export ANTHROPIC_API_KEY=sk-ant-...`
- `export OPENAI_API_KEY=sk-...`
- `export OLLAMA_MODEL=llama3.2` (with Ollama running locally)

---

## Run

```bash
cd /home/propdev/.openclaw/workspace/workspace2/repos/rosa

# ROS_CONTAINER tells rosa to use `docker exec rosa-ros2 ros2 …` for all commands
ROS_CONTAINER=rosa-ros2 \
ANTHROPIC_API_KEY=... \
  cargo run --example turtle -p rosa-cli
```

---

## Demo prompts

Once at the `>` prompt:

```
> What ROS 2 topics are available?
> Move turtle1 forward 3 units.
> Draw a 5-point star using the turtle.
> Set the pen to red (r=255, g=0, b=0) and draw a circle.
```

---

## Turtle reference

| Variable     | Description                          | Default spawn |
|--------------|--------------------------------------|---------------|
| Canvas       | 11.1 × 11.1 units                    | —             |
| Spawn point  | (5.54, 5.54), theta = 0              | Yes           |
| theta = 0    | Facing right (+x direction)          | —             |
| theta = π/2  | Facing up (+y direction)             | —             |
| angular_z > 0| Counterclockwise turn                | —             |

### 5-point star math
- Side length: ~2.5 units
- Turn: **144°** (2.5133 rad) between each leg
- 5 sides, 5 turns → closed star

### Tool set (beyond standard ros2_* tools)

| Tool                        | Description                            |
|-----------------------------|----------------------------------------|
| `turtle_publish_twist`      | Move forward + rotate for N seconds   |
| `turtle_get_pose`           | Read current (x, y, theta)            |
| `turtle_teleport_absolute`  | Jump to (x, y, theta) without drawing |
| `turtle_set_pen`            | Change pen color/width/on-off         |
| `turtle_clear`              | Erase all lines on canvas             |
| `turtle_within_bounds`      | Check if (x, y) is inside 11.1×11.1  |

---

## Troubleshooting

**`ros2` not found**: Set `ROS_CONTAINER=rosa-ros2` to route via `docker exec`.

**Topics not visible**: Ensure the ROS container is on `network_mode: host` (already set in Monoclaw/Deployments/Stacks/ROS/docker-compose.yml).

**TurtleSim window not opening**: X11 forwarding must be enabled. Run `xhost +local:docker` on the host first.
