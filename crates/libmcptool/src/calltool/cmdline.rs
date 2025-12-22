use std::collections::HashMap;

use tmcp::Arguments;

use crate::{Result, output::Output};

/// Parses command line arguments in the form `key=value`.
pub fn parse_command_line_arguments(
    args: Vec<String>,
    output: &Output,
) -> Result<Option<Arguments>> {
    if args.is_empty() {
        return Ok(None);
    }

    let mut arg_map = HashMap::new();
    for arg in args {
        let parts: Vec<&str> = arg.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(crate::Error::Other(format!(
                "Invalid argument format: '{}'. Expected 'key=value'",
                arg
            )));
        }
        let key = parts[0].to_string();
        let value = parts[1].to_string();

        // Try to parse as different types
        let json_value = if value == "true" || value == "false" {
            serde_json::Value::Bool(value.parse().unwrap())
        } else if let Ok(num) = value.parse::<i64>() {
            // Check if the string representation would be different after parsing
            // This catches cases like "00123" which should remain as strings
            if num.to_string() == value {
                serde_json::Value::Number(serde_json::Number::from(num))
            } else {
                serde_json::Value::String(value)
            }
        } else if let Ok(num) = value.parse::<f64>() {
            serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap())
        } else {
            serde_json::Value::String(value)
        };

        arg_map.insert(key, json_value);
    }

    output.trace_info(format!("Parsed arguments: {:?}", arg_map))?;
    Ok(Some(Arguments::from(arg_map)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::Output;

    fn create_test_output() -> Output {
        Output::new(false, 80)
    }

    #[test]
    fn test_empty_args() {
        let output = create_test_output();
        let result = parse_command_line_arguments(vec![], &output).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_string_argument() {
        let output = create_test_output();
        let args = vec!["name=test".to_string()];
        let result = parse_command_line_arguments(args, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("name").unwrap(),
            &serde_json::Value::String("test".to_string())
        );
    }

    #[test]
    fn test_boolean_arguments() {
        let output = create_test_output();
        let args = vec!["flag1=true".to_string(), "flag2=false".to_string()];
        let result = parse_command_line_arguments(args, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("flag1").unwrap(), &serde_json::Value::Bool(true));
        assert_eq!(map.get("flag2").unwrap(), &serde_json::Value::Bool(false));
    }

    #[test]
    fn test_integer_argument() {
        let output = create_test_output();
        let args = vec!["count=42".to_string(), "negative=-10".to_string()];
        let result = parse_command_line_arguments(args, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(
            map.get("count").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(42))
        );
        assert_eq!(
            map.get("negative").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(-10))
        );
    }

    #[test]
    fn test_float_argument() {
        let output = create_test_output();
        let args = vec!["rate=8.14".to_string(), "zero=0.0".to_string()];
        let result = parse_command_line_arguments(args, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(
            map.get("rate").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from_f64(8.14).unwrap())
        );
        assert_eq!(
            map.get("zero").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from_f64(0.0).unwrap())
        );
    }

    #[test]
    fn test_mixed_arguments() {
        let output = create_test_output();
        let args = vec![
            "name=test".to_string(),
            "enabled=true".to_string(),
            "count=100".to_string(),
            "rate=2.5".to_string(),
        ];
        let result = parse_command_line_arguments(args, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 4);
        assert_eq!(
            map.get("name").unwrap(),
            &serde_json::Value::String("test".to_string())
        );
        assert_eq!(map.get("enabled").unwrap(), &serde_json::Value::Bool(true));
        assert_eq!(
            map.get("count").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(100))
        );
        assert_eq!(
            map.get("rate").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from_f64(2.5).unwrap())
        );
    }

    #[test]
    fn test_string_with_equals() {
        let output = create_test_output();
        let args = vec!["url=https://example.com/path?param=value".to_string()];
        let result = parse_command_line_arguments(args, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("url").unwrap(),
            &serde_json::Value::String("https://example.com/path?param=value".to_string())
        );
    }

    #[test]
    fn test_empty_value() {
        let output = create_test_output();
        let args = vec!["empty=".to_string()];
        let result = parse_command_line_arguments(args, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("empty").unwrap(),
            &serde_json::Value::String("".to_string())
        );
    }

    #[test]
    fn test_numeric_string() {
        let output = create_test_output();
        // Numbers that should be parsed as strings due to formatting
        let args = vec!["version=1.0.0".to_string(), "id=00123".to_string()];
        let result = parse_command_line_arguments(args, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(
            map.get("version").unwrap(),
            &serde_json::Value::String("1.0.0".to_string())
        );
        assert_eq!(
            map.get("id").unwrap(),
            &serde_json::Value::String("00123".to_string())
        );
    }

    #[test]
    fn test_invalid_format_no_equals() {
        let output = create_test_output();
        let args = vec!["invalid_arg".to_string()];
        let result = parse_command_line_arguments(args, &output);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid argument format")
        );
    }

    #[test]
    fn test_invalid_format_only_key() {
        let output = create_test_output();
        let args = vec!["key=".to_string(), "another_key".to_string()];
        let result = parse_command_line_arguments(args, &output);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid argument format")
        );
    }

    #[test]
    fn test_key_override() {
        let output = create_test_output();
        let args = vec!["key=first".to_string(), "key=second".to_string()];
        let result = parse_command_line_arguments(args, &output)
            .unwrap()
            .unwrap();

        let map = serde_json::from_value::<HashMap<String, serde_json::Value>>(
            serde_json::to_value(result).unwrap(),
        )
        .unwrap();
        assert_eq!(map.len(), 1);
        // Last value should win
        assert_eq!(
            map.get("key").unwrap(),
            &serde_json::Value::String("second".to_string())
        );
    }
}
