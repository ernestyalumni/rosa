# Task 05 — ROS 2 Bridge

**Owner role:** Rust + ROS agent
**Blocked by:** 02, 04
**Blocks:** 06
**Parallel with:** 03
**Estimate:** 3–4 hr for 5a, 4–6 hr for 5b

This task ships in two waves so phase 6 isn't blocked on r2r mastery.

---

## Phase 5a — CLI shell-out tools (ship first)

Implement `crates/rosa-ros2/` with tools that wrap `ros2 ...` CLI invocations. Parity with upstream `src/rosa/tools/ros2.py` minus the langchain decorator.

### Tools to register

| Tool name              | Wraps                        | Args                                          |
|------------------------|------------------------------|-----------------------------------------------|
| `ros2_list_nodes`      | `ros2 node list`             | `{ pattern: Option<String> }` (regex filter)  |
| `ros2_list_topics`     | `ros2 topic list -t`         | `{ pattern: Option<String> }`                 |
| `ros2_list_services`   | `ros2 service list -t`       | `{ pattern: Option<String> }`                 |
| `ros2_list_params`     | `ros2 param list`            | `{ node: Option<String> }`                    |
| `ros2_topic_echo`      | `ros2 topic echo --once`     | `{ topic: String, msg_type: String }`         |
| `ros2_topic_info`      | `ros2 topic info -v`         | `{ topic: String }`                           |
| `ros2_node_info`       | `ros2 node info`             | `{ node: String }`                            |
| `ros2_param_get`       | `ros2 param get`             | `{ node: String, name: String }`              |
| `ros2_doctor`          | `ros2 doctor --report`       | `{}`                                          |

Honor a constructor blacklist (default `["master", "docker"]` like upstream) — filter the output post-execution.

### Implementation

`tokio::process::Command::new("ros2").args([...]).output().await`. 5-sec timeout via `tokio::time::timeout` on each call. Capture stderr, surface in error.

### Deps

```toml
rosa-core = { path = "../rosa-core" }
rosa-tools = { path = "../rosa-tools" }
async-trait = "0.1"
tokio = { version = "1", features = ["process", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "0.8"
regex = "1"
thiserror = "1"
tracing = "0.1"

[dev-dependencies]
# Integration test below spins up the docker compose ROS stack
```

### Acceptance criteria (5a)

- [ ] `cargo test -p rosa-ros2` unit tests pass (use a `MockCommand` trait so tests don't need a live ROS install).
- [ ] Integration test (`#[ignore]` by default): with `Monoclaw/Deployments/ROS` running, `cargo test -p rosa-ros2 --test integration -- --ignored` enumerates topics and finds `/parameter_events`.
- [ ] Each tool registered with `ToolRegistry` and shows up in `rosa tools` output (once task 06 lands).
- [ ] Blacklist filter unit-tested with a fixture stdout containing `/docker_bridge_topic` → omitted from result.

---

## Phase 5b — Native r2r bridge (follow-up)

Wrap the [`r2r` crate](https://github.com/sequenceplanner/r2r) (Rust ROS 2 client built on `rcl`) for the operations that benefit from being in-process: publishing `geometry_msgs/Twist`, subscribing to `sensor_msgs/Imu`, calling `turtlesim/srv/Spawn`. These power the turtle and Starship demos without a CLI fork-exec per message.

### Tools to add

| Tool name              | r2r operation                                                |
|------------------------|--------------------------------------------------------------|
| `ros2_publish_twist`   | publisher on `{topic}` with `geometry_msgs/msg/Twist`        |
| `ros2_subscribe_once`  | one-shot subscription, returns first message as JSON         |
| `ros2_call_service`    | typed service client                                         |
| `ros2_set_param`       | `rcl_interfaces/srv/SetParameters`                           |

### Constraints

- r2r requires a sourced ROS 2 install at build time. Put `r2r` behind a `ros2-native` Cargo feature so `rosa-cli` can build without ROS 2 on the host (CLI-only mode still works via 5a).
- Provide a Dockerfile in `crates/rosa-ros2/Dockerfile` matching `Monoclaw/Deployments/ROS/Dockerfile`'s base image so the integration test environment is reproducible.

### Acceptance criteria (5b)

- [ ] `cargo build -p rosa-ros2 --features ros2-native` succeeds inside the ROS 2 docker container.
- [ ] Integration test publishes a `Twist` to `/turtle1/cmd_vel` and observes the turtle pose change.
- [ ] Without the `ros2-native` feature, `cargo build -p rosa-ros2` still works (only 5a tools registered).

---

## Out of scope

- ROS 1 — dropped entirely (Noetic EOL'd April 2025).
- Action clients (`ros2 action`) — defer; turtle/Starship demos don't need them.
- DDS configuration knobs — use defaults inside the container.
