use std::{
    collections::HashMap,
    io::{self, BufRead},
};

use tmcp::Arguments;

use crate::{Result, output::Output};

/// Parses JSON arguments from stdin.
pub fn parse_json_arguments(output: &Output) -> Result<Option<Arguments>> {
    parse_json_arguments_from_reader(io::stdin().lock(), output)
}

/// Internal implementation that accepts a reader for testing.
fn parse_json_arguments_from_reader<R: BufRead>(
    reader: R,
    output: &Output,
) -> Result<Option<Arguments>> {
    output.text("Reading JSON arguments from stdin...")?;

    let mut buffer = String::new();
    let mut consecutive_new_lines_count = 0u8;

    for line in reader.lines() {
        let line = line.map_err(|e| crate::Error::Other(format!("Failed to read stdin: {}", e)))?;
        buffer.push_str(&line);

        // Try to parse the accumulated buffer as JSON
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&buffer) {
            return match json_value {
                serde_json::Value::Object(map) => {
                    output.trace_info(format!("Parsed JSON arguments: {:?}", map))?;
                    let map: HashMap<String, serde_json::Value> = map.into_iter().collect();
                    Ok(Some(Arguments::from(map)))
                }
                _ => Err(crate::Error::Other(
                    "JSON input must be an object".to_string(),
                )),
            };
        } else if line.trim().is_empty() {
            consecutive_new_lines_count += 1;
            if consecutive_new_lines_count > 1 {
                break;
            }
        } else {
            consecutive_new_lines_count = 0;
        }
    }

    if buffer.trim().is_empty() {
        Ok(None)
    } else {
        match serde_json::from_str::<serde_json::Value>(&buffer) {
            Ok(_) => unreachable!("JSON should have been parsed earlier"),
            Err(e) => Err(crate::Error::Other(format!("Invalid JSON: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::output::Output;

    fn create_test_output() -> Output {
        Output::new(false, 80)
    }

    #[test]
    fn test_empty_input() {
        let output = create_test_output();
        let input = "";
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_simple_json_object() {
        let output = create_test_output();
        let input = r#"{"name": "test", "count": 42}"#;
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(
            map.get("name").unwrap(),
            &serde_json::Value::String("test".to_string())
        );
        assert_eq!(
            map.get("count").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(42))
        );
    }

    #[test]
    fn test_multiline_json() {
        let output = create_test_output();
        let input = "{\n  \"name\": \"test\",\n  \"enabled\": true\n}";
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(
            map.get("name").unwrap(),
            &serde_json::Value::String("test".to_string())
        );
        assert_eq!(map.get("enabled").unwrap(), &serde_json::Value::Bool(true));
    }

    #[test]
    fn test_complex_json_object() {
        let output = create_test_output();
        let input = r#"{
            "string_val": "hello",
            "int_val": 123,
            "float_val": 3.14,
            "bool_val": true,
            "null_val": null,
            "array_val": [1, 2, 3],
            "nested_obj": {
                "inner": "value"
            }
        }"#;
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 7);
        assert_eq!(
            map.get("string_val").unwrap(),
            &serde_json::Value::String("hello".to_string())
        );
        assert_eq!(
            map.get("int_val").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(123))
        );
        assert_eq!(map.get("bool_val").unwrap(), &serde_json::Value::Bool(true));
        assert_eq!(map.get("null_val").unwrap(), &serde_json::Value::Null);
        assert!(map.get("array_val").unwrap().is_array());
        assert!(map.get("nested_obj").unwrap().is_object());
    }

    #[test]
    fn test_json_array_rejection() {
        let output = create_test_output();
        let input = r#"[1, 2, 3]"#;
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("JSON input must be an object")
        );
    }

    #[test]
    fn test_json_primitive_rejection() {
        let output = create_test_output();
        let input = r#""just a string""#;
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("JSON input must be an object")
        );
    }

    #[test]
    fn test_json_number_rejection() {
        let output = create_test_output();
        let input = "42";
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("JSON input must be an object")
        );
    }

    #[test]
    fn test_json_boolean_rejection() {
        let output = create_test_output();
        let input = "true";
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("JSON input must be an object")
        );
    }

    #[test]
    fn test_invalid_json() {
        let output = create_test_output();
        let input = r#"{invalid json}"#;
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid JSON"));
    }

    #[test]
    fn test_incomplete_json() {
        let output = create_test_output();
        let input = r#"{"name": "test""#; // Missing closing brace
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid JSON"));
    }

    #[test]
    fn test_empty_json_object() {
        let output = create_test_output();
        let input = "{}";
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_whitespace_only_input() {
        let output = create_test_output();
        let input = "\n\n  \t  \n\n";
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_two_consecutive_newlines_stops_reading() {
        let output = create_test_output();
        let input = "incomplete json\n\n{\"this\": \"should not be read\"}";
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid JSON"));
    }

    #[test]
    fn test_json_with_special_characters() {
        let output = create_test_output();
        let input =
            r#"{"unicode": "ñoño 🚀", "escaped": "line1\nline2\ttab", "path": "C:\\Users\\test"}"#;
        let reader = Cursor::new(input);
        let result = parse_json_arguments_from_reader(reader, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get("unicode").unwrap(),
            &serde_json::Value::String("ñoño 🚀".to_string())
        );
        assert_eq!(
            map.get("escaped").unwrap(),
            &serde_json::Value::String("line1\nline2\ttab".to_string())
        );
        assert_eq!(
            map.get("path").unwrap(),
            &serde_json::Value::String("C:\\Users\\test".to_string())
        );
    }
}
