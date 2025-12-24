//! Integration tests for JSON syntax highlighting functionality.
#![allow(clippy::tests_outside_test_module)]

use libmcptool::output::Output;
use serde_json::json;

#[test]
fn test_json_syntax_highlighting() {
    // Test with color output enabled
    let output = Output::new(true, 80);

    let test_data = json!({
        "name": "mcptool",
        "version": "0.1.0",
        "features": ["syntax-highlighting", "json-output"],
        "enabled": true,
        "count": 42
    });

    // This should not panic and should handle both TTY and non-TTY cases
    assert!(output.json_value(&test_data).is_ok());

    // Test with JSON mode (should always output plain JSON)
    let json_output = Output::new(true, 80).with_json(true);
    assert!(json_output.json_value(&test_data).is_ok());
}

#[test]
fn test_output_detects_tty() {
    // Just verify that Output can be created without panic
    let output = Output::new(true, 80);
    let json_output = Output::new(false, 80);

    // Both should be created successfully
    drop(output);
    drop(json_output);
}
