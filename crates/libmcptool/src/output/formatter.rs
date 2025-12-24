use std::fmt;

use crate::{Result, output::Output};

/// Generic trait for formatting output data in both JSON and text modes
pub trait OutputFormatter<T> {
    /// Format data as JSON (default implementation uses json_value)
    fn format_json(&self, output: &Output, data: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        output.json_value(data)
    }

    /// Format data as human-readable text
    fn format_text(&self, output: &Output, data: &T) -> Result<()>;

    /// Choose format based on output mode
    fn format(&self, output: &Output, data: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        if output.json {
            self.format_json(output, data)
        } else {
            self.format_text(output, data)
        }
    }
}

/// Helper function to format output using any formatter
pub fn format_output<T, F>(output: &Output, data: &T, formatter: &F) -> Result<()>
where
    T: serde::Serialize,
    F: OutputFormatter<T>,
{
    formatter.format(output, data)
}

/// Utility struct for simple text formatting
pub struct TextFormatter;

impl<T> OutputFormatter<T> for TextFormatter
where
    T: fmt::Debug,
{
    fn format_text(&self, output: &Output, data: &T) -> Result<()> {
        output.text(format!("{:?}", data))?;
        Ok(())
    }
}

/// Helper for displaying metadata sections commonly used across output modules
pub struct MetadataDisplay;

impl MetadataDisplay {
    pub fn display_uri(output: &Output, uri: &str) -> Result<()> {
        output.text(format!("URI: {}", uri))?;
        Ok(())
    }

    pub fn display_mime_type(output: &Output, mime_type: &Option<String>) -> Result<()> {
        if let Some(mime_type) = mime_type {
            output.text(format!("MIME Type: {}", mime_type))?;
        }
        Ok(())
    }

    pub fn display_description(output: &Output, description: &Option<String>) -> Result<()> {
        if let Some(description) = description {
            output.text(format!("Description: {}", description))?;
        }
        Ok(())
    }

    pub fn display_title(output: &Output, title: &Option<String>) -> Result<()> {
        if let Some(title) = title {
            output.text(format!("Title: {}", title))?;
        }
        Ok(())
    }

    pub fn display_binary_content(output: &Output, data: &str, size: usize) -> Result<()> {
        output.text(format!("Binary content ({} bytes)", size))?;
        output.text("Content (base64):")?;
        output.text(data)?;
        Ok(())
    }

    pub fn display_text_content(output: &Output, text: &str) -> Result<()> {
        output.text("Content:")?;
        output.text(text)?;
        Ok(())
    }
}
