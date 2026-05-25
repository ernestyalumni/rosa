# Deprecated — replaced in feat/rust-rewrite.
#
# The ROS 2 runtime container (formerly this file) now lives at:
#   Monoclaw/Deployments/ROS/Dockerfile
#
# The rosa-cli Rust binary does not need a container of its own — it
# runs on the host and communicates with the ROS 2 container via
# host-network DDS (CycloneDDS).
#
# See: ORCHESTRATION.md and ARCHITECTURE.md
