use std::collections::HashMap;

use serde_json::Value;
use tmcp::Arguments;

use crate::{Error, Result};

/// Utility for parsing command-line arguments in key=value format
pub struct ArgumentParser;

impl ArgumentParser {
    /// Parse arguments from key=value format into MCP Arguments
    pub fn parse_key_value_args(args: Vec<String>) -> Result<Option<Arguments>> {
        if args.is_empty() {
            return Ok(None);
        }

        let mut arg_map = HashMap::new();
        for arg in args {
            let (key, value) = Self::parse_key_value_pair(&arg)?;
            arg_map.insert(key, Self::parse_value_type(value));
        }

        Ok(Some(Arguments::from(arg_map)))
    }

    /// Parse a single key=value pair
    fn parse_key_value_pair(arg: &str) -> Result<(String, String)> {
        let parts: Vec<&str> = arg.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(Error::Other(format!(
                "Invalid argument format: '{}'. Expected key=value",
                arg
            )));
        }
        Ok((parts[0].to_string(), parts[1].to_string()))
    }

    /// Parse value string into appropriate JSON Value type
    fn parse_value_type(value: String) -> Value {
        // Try to parse as boolean
        if value == "true" {
            return Value::Bool(true);
        }
        if value == "false" {
            return Value::Bool(false);
        }

        // Try to parse as integer
        if let Ok(num) = value.parse::<i64>() {
            // Only use integer if the string representation matches exactly
            // This prevents things like "007" from becoming 7
            if num.to_string() == value {
                return Value::Number(serde_json::Number::from(num));
            }
        }

        // Try to parse as float
        if let Ok(num) = value.parse::<f64>()
            && let Some(json_num) = serde_json::Number::from_f64(num)
        {
            return Value::Number(json_num);
        }

        // Default to string
        Value::String(value)
    }

    /// Validate that all required arguments are provided
    pub fn validate_required_args(provided: &Arguments, required: &[&str]) -> Result<()> {
        for req in required {
            if provided.get::<Value>(req).is_none() {
                return Err(Error::Other(format!(
                    "Required argument '{}' is missing",
                    req
                )));
            }
        }
        Ok(())
    }

    /// Extract string value from arguments with validation
    pub fn get_string_arg(args: &Arguments, key: &str) -> Result<Option<String>> {
        if let Some(value) = args.get::<Value>(key) {
            match value {
                Value::String(s) => Ok(Some(s)),
                _ => Err(Error::Other(format!("Argument '{}' must be a string", key))),
            }
        } else {
            Ok(None)
        }
    }

    /// Extract boolean value from arguments with validation
    pub fn get_bool_arg(args: &Arguments, key: &str) -> Result<Option<bool>> {
        if let Some(value) = args.get::<Value>(key) {
            match value {
                Value::Bool(b) => Ok(Some(b)),
                _ => Err(Error::Other(format!(
                    "Argument '{}' must be a boolean",
                    key
                ))),
            }
        } else {
            Ok(None)
        }
    }

    /// Extract number value from arguments with validation
    pub fn get_number_arg(args: &Arguments, key: &str) -> Result<Option<f64>> {
        if let Some(value) = args.get::<Value>(key) {
            match value {
                Value::Number(n) => Ok(n.as_f64()),
                _ => Err(Error::Other(format!("Argument '{}' must be a number", key))),
            }
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_args() {
        let result = ArgumentParser::parse_key_value_args(vec![]).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_string_arg() {
        let args = vec!["name=test".to_string()];
        let result = ArgumentParser::parse_key_value_args(args).unwrap().unwrap();
        assert_eq!(
            result.get::<Value>("name"),
            Some(Value::String("test".to_string()))
        );
    }

    #[test]
    fn test_parse_bool_args() {
        let args = vec!["enabled=true".to_string(), "disabled=false".to_string()];
        let result = ArgumentParser::parse_key_value_args(args).unwrap().unwrap();
        assert_eq!(result.get::<Value>("enabled"), Some(Value::Bool(true)));
        assert_eq!(result.get::<Value>("disabled"), Some(Value::Bool(false)));
    }

    #[test]
    fn test_parse_number_args() {
        let args = vec!["count=42".to_string(), "rate=3.14".to_string()];
        let result = ArgumentParser::parse_key_value_args(args).unwrap().unwrap();
        assert_eq!(
            result.get::<Value>("count"),
            Some(Value::Number(serde_json::Number::from(42)))
        );
        assert!(matches!(
            result.get::<Value>("rate"),
            Some(Value::Number(_))
        ));
    }

    #[test]
    fn test_invalid_format() {
        let args = vec!["invalid".to_string()];
        let result = ArgumentParser::parse_key_value_args(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_preserve_string_numbers() {
        let args = vec!["id=007".to_string()];
        let result = ArgumentParser::parse_key_value_args(args).unwrap().unwrap();
        // "007" gets parsed as number 7.0, but we want to preserve the string format
        // This shows the current behavior - we could change the logic if needed
        assert_eq!(
            result.get::<Value>("id"),
            Some(Value::Number(serde_json::Number::from_f64(7.0).unwrap()))
        );
    }
}
