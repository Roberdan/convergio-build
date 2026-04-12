//! MCP tool definitions for the build extension.

use convergio_types::extension::McpToolDef;
use serde_json::json;

pub fn build_tools() -> Vec<McpToolDef> {
    vec![
        McpToolDef {
            name: "cvg_build_self".into(),
            description: "Trigger a self-build of the daemon.".into(),
            method: "POST".into(),
            path: "/api/build/self".into(),
            input_schema: json!({"type": "object", "properties": {}}),
            min_ring: "core".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_build_status".into(),
            description: "Get status of a build.".into(),
            method: "GET".into(),
            path: "/api/build/status/:id".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"id": {"type": "string"}},
                "required": ["id"]
            }),
            min_ring: "community".into(),
            path_params: vec!["id".into()],
        },
        McpToolDef {
            name: "cvg_build_history".into(),
            description: "Get build history.".into(),
            method: "GET".into(),
            path: "/api/build/history".into(),
            input_schema: json!({"type": "object", "properties": {}}),
            min_ring: "community".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_build_deploy".into(),
            description: "Deploy a specific build.".into(),
            method: "POST".into(),
            path: "/api/build/deploy/:id".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"id": {"type": "string"}},
                "required": ["id"]
            }),
            min_ring: "core".into(),
            path_params: vec!["id".into()],
        },
        McpToolDef {
            name: "cvg_build_rollback".into(),
            description: "Rollback to a previous build.".into(),
            method: "POST".into(),
            path: "/api/build/rollback".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "build_id": {"type": "string", "description": "Build ID to rollback to"}
                }
            }),
            min_ring: "core".into(),
            path_params: vec![],
        },
    ]
}
