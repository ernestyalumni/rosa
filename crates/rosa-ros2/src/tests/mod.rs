//! Unit tests for rosa-ros2.
//!
//! Uses `MockRunner` to avoid requiring a live ROS 2 install.
//! Integration tests that need the real `ros2` CLI are marked `#[ignore]` and
//! can be run with:
//!   `cargo test -p rosa-ros2 -- --include-ignored`

use serde_json::json;
use rosa_core::agent::ToolDispatcher;
use rosa_tools::Tool;

use crate::{
    filter::filter_lines,
    runner::MockRunner,
    tools::{
        DoctorTool, ListNodesTool, ListServicesTool, ListTopicsTool,
        ParamSetTool, ServiceCallTool, ServiceInfoTool, TopicEchoTool,
    },
    ros2_registry_default,
};

// ---------------------------------------------------------------------------
// filter::filter_lines (tested in filter.rs; extra cross-tool test here)
// ---------------------------------------------------------------------------

#[test]
fn test_blacklist_removes_docker_and_master() {
    let output = "/rosout\n/master_node\n/turtlesim\n/docker_bridge_topic\n";
    let blacklist = vec!["master".into(), "docker".into()];
    let result = filter_lines(output, &blacklist, None);
    assert_eq!(result, vec!["/rosout", "/turtlesim"]);
}

// ---------------------------------------------------------------------------
// ros2_list_nodes (with MockRunner)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_nodes_returns_filtered_nodes() {
    let runner = MockRunner::new("/rosout\n/master_node\n/turtlesim\n/docker_bridge\n");
    let tool = ListNodesTool::with_runner(runner, vec!["master".into(), "docker".into()]);
    let result = tool.execute(json!({})).await.unwrap();

    let nodes: Vec<&str> = result["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(nodes.contains(&"/rosout"),    "expected /rosout");
    assert!(nodes.contains(&"/turtlesim"), "expected /turtlesim");
    assert!(!nodes.iter().any(|n| n.contains("master")), "master should be filtered");
    assert!(!nodes.iter().any(|n| n.contains("docker")), "docker should be filtered");
}

#[tokio::test]
async fn test_list_nodes_pattern_filter() {
    let runner = MockRunner::new("/rosout\n/turtlesim\n/my_node\n");
    let tool = ListNodesTool::with_runner(runner, vec![]);
    let result = tool.execute(json!({"pattern": "turtle"})).await.unwrap();

    let nodes: Vec<&str> = result["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(nodes, vec!["/turtlesim"]);
}

// ---------------------------------------------------------------------------
// ros2_list_topics (with MockRunner)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_topics_parses_name_and_type() {
    let output = "/clock [std_msgs/msg/Time]\n\
                  /parameter_events [rcl_interfaces/msg/ParameterEvent]\n\
                  /docker_topic [std_msgs/msg/String]\n";
    let runner = MockRunner::new(output);
    let tool = ListTopicsTool::with_runner(runner, vec!["docker".into()]);
    let result = tool.execute(json!({})).await.unwrap();

    let topics = result["topics"].as_array().unwrap();
    assert_eq!(topics.len(), 2);
    assert_eq!(topics[0]["name"], json!("/clock"));
    assert_eq!(topics[0]["type"], json!("std_msgs/msg/Time"));
    assert_eq!(topics[1]["name"], json!("/parameter_events"));
}

// ---------------------------------------------------------------------------
// ros2_list_services (with MockRunner)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_services_parses_correctly() {
    let output = "/turtlesim/spawn [turtlesim/srv/Spawn]\n";
    let runner = MockRunner::new(output);
    let tool = ListServicesTool::with_runner(runner, vec![]);
    let result = tool.execute(json!({})).await.unwrap();

    let services = result["services"].as_array().unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0]["name"], json!("/turtlesim/spawn"));
    assert_eq!(services[0]["type"], json!("turtlesim/srv/Spawn"));
}

// ---------------------------------------------------------------------------
// ros2_topic_echo (with MockRunner)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_topic_echo_returns_message() {
    let yaml = "x: 5.54\ny: 5.54\ntheta: 0.0\n";
    let runner = MockRunner::new(yaml);
    let tool = TopicEchoTool::with_runner(runner, vec![]);
    let result = tool
        .execute(json!({"topic": "/turtle1/pose", "msg_type": "turtlesim/msg/Pose"}))
        .await
        .unwrap();
    assert!(result["message"].as_str().unwrap().contains("x: 5.54"));
}

// ---------------------------------------------------------------------------
// ros2_param_set (with MockRunner)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_param_set_returns_result() {
    let runner = MockRunner::new("Set parameter successful");
    let tool = ParamSetTool::with_runner(runner, vec![]);
    let result = tool
        .execute(serde_json::json!({
            "node": "/turtlesim",
            "name": "background_r",
            "value": "255"
        }))
        .await
        .unwrap();
    assert_eq!(result["node"], serde_json::json!("/turtlesim"));
    assert_eq!(result["name"], serde_json::json!("background_r"));
    assert_eq!(result["value"], serde_json::json!("255"));
    assert!(result["result"].as_str().unwrap().contains("successful"));
}

// ---------------------------------------------------------------------------
// ros2_service_call (with MockRunner)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_service_call_returns_response() {
    let runner = MockRunner::new("response:\n  name: turtle2");
    let tool = ServiceCallTool::with_runner(runner, vec![]);
    let result = tool
        .execute(serde_json::json!({
            "service": "/turtlesim/spawn",
            "srv_type": "turtlesim/srv/Spawn",
            "request": "{x: 2.0, y: 2.0, theta: 0.0, name: 'turtle2'}"
        }))
        .await
        .unwrap();
    assert_eq!(result["service"], serde_json::json!("/turtlesim/spawn"));
    assert!(result["response"].as_str().unwrap().contains("turtle2"));
}

#[tokio::test]
async fn test_service_call_defaults_empty_request() {
    let runner = MockRunner::new("response: {}");
    let tool = ServiceCallTool::with_runner(runner, vec![]);
    // No "request" field — should default to "{}" without error
    let result = tool
        .execute(serde_json::json!({
            "service": "/turtlesim/clear",
            "srv_type": "std_srvs/srv/Empty"
        }))
        .await
        .unwrap();
    assert_eq!(result["service"], serde_json::json!("/turtlesim/clear"));
}

// ---------------------------------------------------------------------------
// ros2_service_info (with MockRunner)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_service_info_returns_type_for_each_service() {
    let runner = MockRunner::new("turtlesim/srv/Spawn");
    let tool = ServiceInfoTool::with_runner(runner, vec![]);
    let result = tool
        .execute(serde_json::json!({
            "services": ["/turtlesim/spawn"]
        }))
        .await
        .unwrap();
    assert_eq!(
        result["/turtlesim/spawn"].as_str().unwrap(),
        "turtlesim/srv/Spawn"
    );
}

#[tokio::test]
async fn test_service_info_handles_multiple_services() {
    // MockRunner returns the same string for every call
    let runner = MockRunner::new("std_srvs/srv/Empty");
    let tool = ServiceInfoTool::with_runner(runner, vec![]);
    let result = tool
        .execute(serde_json::json!({
            "services": ["/turtlesim/clear", "/turtlesim/reset"]
        }))
        .await
        .unwrap();
    assert!(result["/turtlesim/clear"].is_string());
    assert!(result["/turtlesim/reset"].is_string());
}

// ---------------------------------------------------------------------------
// ros2_doctor (with MockRunner)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_doctor_returns_report() {
    let report = "All 5 checks passed\n";
    let runner = MockRunner::new(report);
    let tool = DoctorTool::with_runner(runner, vec![]);
    let result = tool.execute(json!({})).await.unwrap();
    assert!(result["report"].as_str().unwrap().contains("All 5 checks passed"));
}

// ---------------------------------------------------------------------------
// Registry registration
// ---------------------------------------------------------------------------

#[test]
fn test_ros2_registry_registers_all_tools() {
    let registry = ros2_registry_default();
    assert_eq!(registry.len(), 13);
    let specs = registry.tool_specs();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"ros2_list_nodes"));
    assert!(names.contains(&"ros2_list_topics"));
    assert!(names.contains(&"ros2_list_services"));
    assert!(names.contains(&"ros2_list_params"));
    assert!(names.contains(&"ros2_topic_echo"));
    assert!(names.contains(&"ros2_topic_info"));
    assert!(names.contains(&"ros2_node_info"));
    assert!(names.contains(&"ros2_param_get"));
    assert!(names.contains(&"ros2_param_set"));
    assert!(names.contains(&"ros2_service_call"));
    assert!(names.contains(&"ros2_service_info"));
    assert!(names.contains(&"ros2_doctor"));
    assert!(names.contains(&"roslog_list"));
}

#[test]
fn test_registry_as_openai_tools() {
    let registry = ros2_registry_default();
    let tools = registry.as_openai_tools();
    assert_eq!(tools.len(), 13);
    for t in &tools {
        assert_eq!(t["type"], json!("function"));
        assert!(t["function"]["name"].is_string());
        assert!(t["function"]["description"].is_string());
        assert!(t["function"]["parameters"]["type"] == json!("object") ||
                t["function"]["parameters"].is_object());
    }
}

// ---------------------------------------------------------------------------
// Integration tests (require live ROS 2 container — run with --include-ignored)
// ---------------------------------------------------------------------------
//
// rosa does NOT install ros2 on the host. All ROS 2 commands go through
// `docker exec` when `ROS_CONTAINER` is set. Always run integration tests
// with that variable:
//
//   cd Monoclaw/Deployments/ROS && docker compose up -d
//   ROS_CONTAINER=rosa-ros2 cargo test -p rosa-ros2 -- --include-ignored
//
// Without ROS_CONTAINER the ShellRunner tries to exec `ros2` on the host
// and gets "No such file or directory".

/// Requires the `Monoclaw/Deployments/ROS` docker-compose stack running
/// and `ROS_CONTAINER=rosa-ros2` set in the environment.
#[tokio::test]
#[ignore = "requires: docker compose up -d (ROS) + ROS_CONTAINER=rosa-ros2 env var"]
async fn test_integration_list_topics_finds_parameter_events() {
    use crate::tools::ListTopicsTool;
    let tool = ListTopicsTool::new(vec![]);
    let result = tool.execute(json!({})).await.unwrap();
    let topics = result["topics"].as_array().unwrap();
    let names: Vec<&str> = topics
        .iter()
        .map(|t| t["name"].as_str().unwrap_or(""))
        .collect();
    assert!(
        names.contains(&"/parameter_events"),
        "expected /parameter_events in topic list, got: {names:?}"
    );
}

#[tokio::test]
#[ignore = "requires: docker compose up -d (ROS) + ROS_CONTAINER=rosa-ros2 env var"]
async fn test_integration_doctor_passes() {
    use crate::tools::DoctorTool;
    let tool = DoctorTool::new(vec![]);
    let result = tool.execute(json!({})).await.unwrap();
    let report = result["report"].as_str().unwrap();
    // `ros2 doctor --report` emits a structured diagnostic dump with named sections
    // (NETWORK CONFIGURATION, PLATFORM INFORMATION, ROS 2 INFORMATION, etc.).
    // It does NOT print "ok" or "passed" — that's what bare `ros2 doctor` (no flag) says.
    // Assert the report is non-empty and contains the ROS 2 section header.
    assert!(!report.is_empty(), "doctor report was empty");
    assert!(
        report.contains("ROS 2 INFORMATION") || report.contains("PLATFORM INFORMATION"),
        "expected structured --report output, got:\n{report}"
    );
    // Confirm the RMW middleware matches our CycloneDDS config.
    assert!(
        report.contains("rmw_cyclonedds_cpp"),
        "expected CycloneDDS middleware, got:\n{report}"
    );
}
