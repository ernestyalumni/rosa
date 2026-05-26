//! Isaac Sim tool implementations.

mod diagnostics;
mod scene;
mod timeline;

pub use diagnostics::GetDiagnosticsTool;
pub use scene::{CreateStarshipStageTool, ListUsdsTool, LoadUsdTool, StarshipStageStatusTool};
pub use timeline::{TimelinePauseTool, TimelineStartTool, TimelineStopTool};

use rosa_tools::ToolRegistry;

use crate::IsaacClient;

/// Register all Isaac Sim tools into a registry.
///
/// ```no_run
/// use rosa_isaac::{IsaacClient, tools::all_isaac_tools};
/// use rosa_tools::ToolRegistry;
///
/// let registry = all_isaac_tools(ToolRegistry::new(), IsaacClient::default());
/// ```
pub fn all_isaac_tools(registry: ToolRegistry, client: IsaacClient) -> ToolRegistry {
    registry
        .register(TimelineStartTool::new(client.clone()))
        .register(TimelineStopTool::new(client.clone()))
        .register(TimelinePauseTool::new(client.clone()))
        .register(GetDiagnosticsTool::new(client.clone()))
        .register(LoadUsdTool::new(client.clone()))
        .register(ListUsdsTool::new(client.clone()))
        .register(CreateStarshipStageTool::new(client.clone()))
        .register(StarshipStageStatusTool::new(client))
}
