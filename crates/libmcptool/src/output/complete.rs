use tmcp::schema::CompleteResult;

use super::formatter::{OutputFormatter, format_output};
use crate::Result;

pub struct CompleteFormatter;

impl OutputFormatter<CompleteResult> for CompleteFormatter {
    fn format_text(&self, output: &crate::output::Output, result: &CompleteResult) -> Result<()> {
        output.text(format!(
            "Completions ({} item(s)):",
            result.completion.values.len()
        ))?;

        for (i, completion) in result.completion.values.iter().enumerate() {
            output.text(format!("  {}. {}", i + 1, completion))?;
        }

        if result.completion.has_more.unwrap_or(false) {
            output.text("  ... (more completions available)")?;
        }

        if let Some(total) = result.completion.total {
            output.text(format!("Total completions: {}", total))?;
        }

        Ok(())
    }
}

pub fn complete_result(output: &crate::output::Output, result: &CompleteResult) -> Result<()> {
    format_output(output, result, CompleteFormatter)
}
