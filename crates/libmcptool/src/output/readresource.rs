use tmcp::schema::{self, ReadResourceResult};

use crate::output::formatter::{MetadataDisplay, OutputFormatter, format_output};
use crate::{Result, output::Output};

pub struct ReadResourceFormatter;

impl OutputFormatter<ReadResourceResult> for ReadResourceFormatter {
    fn format_text(&self, output: &Output, result: &ReadResourceResult) -> Result<()> {
        output.text(format!(
            "Resource contents ({} item(s)):",
            result.contents.len()
        ))?;

        for (i, content) in result.contents.iter().enumerate() {
            output.text(format!("\n--- Content {} ---", i + 1))?;
            match content {
                schema::ResourceContents::Text(text_resource) => {
                    MetadataDisplay::display_uri(output, &text_resource.uri)?;
                    MetadataDisplay::display_mime_type(output, &text_resource.mime_type)?;
                    MetadataDisplay::display_text_content(output, &text_resource.text)?;
                }
                schema::ResourceContents::Blob(blob_resource) => {
                    MetadataDisplay::display_uri(output, &blob_resource.uri)?;
                    MetadataDisplay::display_mime_type(output, &blob_resource.mime_type)?;
                    MetadataDisplay::display_binary_content(
                        output,
                        &blob_resource.blob,
                        blob_resource.blob.len(),
                    )?;
                }
            }
        }
        Ok(())
    }
}

pub fn read_resource_result(output: &Output, result: &ReadResourceResult) -> Result<()> {
    format_output(output, result, &ReadResourceFormatter)
}
