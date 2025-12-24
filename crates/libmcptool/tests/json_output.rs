//! Integration tests for JSON output formatting.
#![allow(clippy::tests_outside_test_module)]

use std::collections::HashMap;

use libmcptool::output::{Output, listtools};
use serde_json::json;
use tmcp::schema::{ListToolsResult, Tool, ToolSchema};

#[test]
fn test_list_tools_result_json_output() {
    // Create a mock ListToolsResult
    let mut properties = HashMap::new();
    properties.insert(
        "param1".to_string(),
        json!({
            "type": "string",
            "description": "First parameter"
        }),
    );

    let input_schema = ToolSchema {
        schema_type: "object".to_string(),
        properties: Some(properties),
        required: Some(vec!["param1".to_string()]),
    };

    let tool = Tool::new("test_tool", input_schema).with_description("A test tool");

    let tools_result = ListToolsResult {
        tools: vec![tool],
        next_cursor: None,
    };

    // Test JSON output
    let json_output = Output::new(false, 80).with_json(true);
    let result = listtools::list_tools_result(&json_output, &tools_result);
    assert!(result.is_ok());

    // Test text output
    let text_output = Output::new(true, 80).with_json(false);
    let result = listtools::list_tools_result(&text_output, &tools_result);
    assert!(result.is_ok());
}

#[test]
fn test_list_tools_result_empty_tools() {
    // Create an empty ListToolsResult
    let tools_result = ListToolsResult {
        tools: vec![],
        next_cursor: None,
    };

    // Test JSON output with empty tools
    let json_output = Output::new(false, 80).with_json(true);
    let result = listtools::list_tools_result(&json_output, &tools_result);
    assert!(result.is_ok());

    // Test text output with empty tools
    let text_output = Output::new(true, 80).with_json(false);
    let result = listtools::list_tools_result(&text_output, &tools_result);
    assert!(result.is_ok());
}

#[test]
fn test_ping_output() {
    // Test JSON output
    let json_output = Output::new(false, 80).with_json(true);
    let result = json_output.ping();
    assert!(result.is_ok());

    // Test text output
    let text_output = Output::new(true, 80).with_json(false);
    let result = text_output.ping();
    assert!(result.is_ok());
}
