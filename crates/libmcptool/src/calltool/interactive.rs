use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use tmcp::Arguments;

use crate::Result;

pub fn parse_interactive_arguments(
    tool: &tmcp::schema::Tool,
    output: &crate::output::Output,
) -> Result<Option<Arguments>> {
    parse_interactive_arguments_with_io(tool, output, &mut io::stdin().lock(), &mut io::stdout())
}

fn parse_interactive_arguments_with_io<R: BufRead, W: Write>(
    tool: &tmcp::schema::Tool,
    output: &crate::output::Output,
    reader: &mut R,
    writer: &mut W,
) -> Result<Option<Arguments>> {
    let _ = output.text("Interactive mode: Enter tool parameters");

    let properties = tool.input_schema.properties.as_ref();
    let empty_vec = vec![];
    let required = tool.input_schema.required.as_ref().unwrap_or(&empty_vec);

    if properties.is_none() {
        let _ = output.text("No parameters required for this tool");
        return Ok(None);
    }

    let properties = properties.unwrap();
    let mut arg_map = HashMap::new();

    // Sort parameters by name for deterministic order in tests
    let mut sorted_params: Vec<_> = properties.iter().collect();
    sorted_params.sort_by_key(|(name, _)| *name);

    for (param_name, param_schema) in sorted_params {
        let is_required = required.contains(param_name);
        let param_type = param_schema
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");
        let description = param_schema
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");

        let prompt = if is_required {
            format!("{} ({})*: {}", param_name, param_type, description)
        } else {
            format!(
                "{} ({}) [optional]: {}",
                param_name, param_type, description
            )
        };

        loop {
            writeln!(writer, "{}", prompt)
                .map_err(|e| crate::Error::Other(format!("Failed to write prompt: {}", e)))?;
            write!(writer, "> ")
                .map_err(|e| crate::Error::Other(format!("Failed to write prompt: {}", e)))?;
            writer
                .flush()
                .map_err(|e| crate::Error::Other(format!("Failed to flush stdout: {}", e)))?;

            let mut input = String::new();
            reader
                .read_line(&mut input)
                .map_err(|e| crate::Error::Other(format!("Failed to read input: {}", e)))?;

            let input = input.trim();

            // Skip optional empty parameters
            if input.is_empty() && !is_required {
                break;
            }

            // Require input for required parameters
            if input.is_empty() && is_required {
                writeln!(writer, "This parameter is required. Please enter a value.")
                    .map_err(|e| crate::Error::Other(format!("Failed to write error: {}", e)))?;
                continue;
            }

            // Parse input based on expected type
            let json_value = match param_type {
                "boolean" => match input.to_lowercase().as_str() {
                    "true" | "t" | "yes" | "y" | "1" => serde_json::Value::Bool(true),
                    "false" | "f" | "no" | "n" | "0" => serde_json::Value::Bool(false),
                    _ => {
                        writeln!(
                            writer,
                            "Invalid boolean value. Use true/false, yes/no, or 1/0."
                        )
                        .map_err(|e| {
                            crate::Error::Other(format!("Failed to write error: {}", e))
                        })?;
                        continue;
                    }
                },
                "integer" => match input.parse::<i64>() {
                    Ok(num) => serde_json::Value::Number(serde_json::Number::from(num)),
                    Err(_) => {
                        writeln!(writer, "Invalid integer value.").map_err(|e| {
                            crate::Error::Other(format!("Failed to write error: {}", e))
                        })?;
                        continue;
                    }
                },
                "number" => match input.parse::<f64>() {
                    Ok(num) => {
                        serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap())
                    }
                    Err(_) => {
                        writeln!(writer, "Invalid number value.").map_err(|e| {
                            crate::Error::Other(format!("Failed to write error: {}", e))
                        })?;
                        continue;
                    }
                },
                _ => serde_json::Value::String(input.to_string()),
            };

            arg_map.insert(param_name.clone(), json_value);
            break;
        }
    }

    if arg_map.is_empty() {
        Ok(None)
    } else {
        let _ = output.trace_info(format!("Interactive arguments: {:?}", arg_map));
        Ok(Some(Arguments::from(arg_map)))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use tmcp::schema::{Tool, ToolSchema};

    use super::*;
    use crate::output::Output;

    fn create_test_output() -> Output {
        Output::new(false, 80)
    }

    fn create_test_tool(
        properties: serde_json::Map<String, serde_json::Value>,
        required: Option<Vec<String>>,
    ) -> Tool {
        let properties_map: std::collections::HashMap<String, serde_json::Value> =
            properties.into_iter().collect();
        Tool {
            name: "test_tool".to_string(),
            title: Some("Test tool".to_string()),
            description: Some("Test tool".to_string()),
            input_schema: ToolSchema {
                schema_type: "object".to_string(),
                properties: Some(properties_map),
                required,
            },
            output_schema: None,
            annotations: None,
            _meta: None,
        }
    }

    #[test]
    fn test_no_properties() {
        let output = create_test_output();
        let tool = create_test_tool(serde_json::Map::new(), None);
        let input = "";
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();

        let result =
            parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_single_string_parameter() {
        let output = create_test_output();
        let mut properties = serde_json::Map::new();
        properties.insert(
            "name".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "User name"
            }),
        );
        let tool = create_test_tool(properties, Some(vec!["name".to_string()]));
        let input = "John\n";
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();

        let result = parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer)
            .unwrap()
            .unwrap();
        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("name").unwrap(),
            &serde_json::Value::String("John".to_string())
        );
    }

    #[test]
    fn test_boolean_parameter_various_inputs() {
        let test_cases = vec![
            ("true", true),
            ("True", true),
            ("TRUE", true),
            ("t", true),
            ("T", true),
            ("yes", true),
            ("YES", true),
            ("y", true),
            ("Y", true),
            ("1", true),
            ("false", false),
            ("False", false),
            ("FALSE", false),
            ("f", false),
            ("F", false),
            ("no", false),
            ("NO", false),
            ("n", false),
            ("N", false),
            ("0", false),
        ];

        for (input_val, expected) in test_cases {
            let output = create_test_output();
            let mut properties = serde_json::Map::new();
            properties.insert(
                "flag".to_string(),
                serde_json::json!({
                    "type": "boolean",
                    "description": "Boolean flag"
                }),
            );
            let tool = create_test_tool(properties, Some(vec!["flag".to_string()]));
            let input = format!("{}\n", input_val);
            let mut reader = Cursor::new(input);
            let mut writer = Vec::new();

            let result =
                parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer)
                    .unwrap()
                    .unwrap();
            let map =
                serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
                    serde_json::to_value(result).unwrap(),
                )
                .unwrap();
            assert_eq!(map.len(), 1);
            assert_eq!(map.get("flag").unwrap(), &serde_json::Value::Bool(expected));
        }
    }

    #[test]
    fn test_integer_parameter() {
        let output = create_test_output();
        let mut properties = serde_json::Map::new();
        properties.insert(
            "count".to_string(),
            serde_json::json!({
                "type": "integer",
                "description": "Count value"
            }),
        );
        let tool = create_test_tool(properties, Some(vec!["count".to_string()]));
        let input = "42\n";
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();

        let result = parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer)
            .unwrap()
            .unwrap();
        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("count").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(42))
        );
    }

    #[test]
    fn test_number_parameter() {
        let output = create_test_output();
        let mut properties = serde_json::Map::new();
        properties.insert(
            "rate".to_string(),
            serde_json::json!({
                "type": "number",
                "description": "Rate value"
            }),
        );
        let tool = create_test_tool(properties, Some(vec!["rate".to_string()]));
        let input = "8.14\n";
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();

        let result = parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer)
            .unwrap()
            .unwrap();
        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("rate").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from_f64(8.14).unwrap())
        );
    }

    #[test]
    fn test_optional_parameter_empty() {
        let output = create_test_output();
        let mut properties = serde_json::Map::new();
        properties.insert(
            "optional".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "Optional parameter"
            }),
        );
        let tool = create_test_tool(properties, None); // No required parameters
        let input = "\n"; // Empty input
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();

        let result =
            parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer).unwrap();
        assert!(result.is_none()); // Should return None for empty arg_map
    }

    #[test]
    fn test_mixed_parameters() {
        let output = create_test_output();
        let mut properties = serde_json::Map::new();
        properties.insert(
            "name".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "User name"
            }),
        );
        properties.insert(
            "count".to_string(),
            serde_json::json!({
                "type": "integer",
                "description": "Count value"
            }),
        );
        properties.insert(
            "enabled".to_string(),
            serde_json::json!({
                "type": "boolean",
                "description": "Enabled flag"
            }),
        );
        let tool = create_test_tool(
            properties,
            Some(vec!["name".to_string(), "count".to_string()]),
        );
        let input = "100\ntrue\nAlice\n";
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();

        let result = parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer)
            .unwrap()
            .unwrap();
        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get("name").unwrap(),
            &serde_json::Value::String("Alice".to_string())
        );
        assert_eq!(
            map.get("count").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(100))
        );
        assert_eq!(map.get("enabled").unwrap(), &serde_json::Value::Bool(true));
    }

    #[test]
    fn test_invalid_boolean_then_valid() {
        let output = create_test_output();
        let mut properties = serde_json::Map::new();
        properties.insert(
            "flag".to_string(),
            serde_json::json!({
                "type": "boolean",
                "description": "Boolean flag"
            }),
        );
        let tool = create_test_tool(properties, Some(vec!["flag".to_string()]));
        let input = "invalid\ntrue\n";
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();

        let result = parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer)
            .unwrap()
            .unwrap();
        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("flag").unwrap(), &serde_json::Value::Bool(true));

        // Check that error message was written
        let output_str = String::from_utf8(writer).unwrap();
        assert!(output_str.contains("Invalid boolean value"));
    }

    #[test]
    fn test_invalid_integer_then_valid() {
        let output = create_test_output();
        let mut properties = serde_json::Map::new();
        properties.insert(
            "count".to_string(),
            serde_json::json!({
                "type": "integer",
                "description": "Count value"
            }),
        );
        let tool = create_test_tool(properties, Some(vec!["count".to_string()]));
        let input = "not_a_number\n42\n";
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();

        let result = parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer)
            .unwrap()
            .unwrap();
        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("count").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(42))
        );

        // Check that error message was written
        let output_str = String::from_utf8(writer).unwrap();
        assert!(output_str.contains("Invalid integer value"));
    }

    #[test]
    fn test_invalid_number_then_valid() {
        let output = create_test_output();
        let mut properties = serde_json::Map::new();
        properties.insert(
            "rate".to_string(),
            serde_json::json!({
                "type": "number",
                "description": "Rate value"
            }),
        );
        let tool = create_test_tool(properties, Some(vec!["rate".to_string()]));
        let input = "not_a_number\n8.14\n";
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();

        let result = parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer)
            .unwrap()
            .unwrap();
        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("rate").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from_f64(8.14).unwrap())
        );

        // Check that error message was written
        let output_str = String::from_utf8(writer).unwrap();
        assert!(output_str.contains("Invalid number value"));
    }

    #[test]
    fn test_default_string_type() {
        let output = create_test_output();
        let mut properties = serde_json::Map::new();
        properties.insert(
            "unknown_type".to_string(),
            serde_json::json!({
                "description": "Unknown type parameter"
                // No "type" field, should default to string
            }),
        );
        let tool = create_test_tool(properties, Some(vec!["unknown_type".to_string()]));
        let input = "test_value\n";
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();

        let result = parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer)
            .unwrap()
            .unwrap();
        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("unknown_type").unwrap(),
            &serde_json::Value::String("test_value".to_string())
        );
    }

    #[test]
    fn test_empty_required_parameter_retry() {
        let output = create_test_output();
        let mut properties = serde_json::Map::new();
        properties.insert(
            "required_param".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "Required parameter"
            }),
        );
        let tool = create_test_tool(properties, Some(vec!["required_param".to_string()]));
        let input = "\nvalid_value\n"; // Empty first, then valid
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();

        let result = parse_interactive_arguments_with_io(&tool, &output, &mut reader, &mut writer)
            .unwrap()
            .unwrap();
        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("required_param").unwrap(),
            &serde_json::Value::String("valid_value".to_string())
        );

        // Check that error message was written
        let output_str = String::from_utf8(writer).unwrap();
        assert!(output_str.contains("This parameter is required"));
    }
}
