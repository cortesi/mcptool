use tmcp::schema::{self, GetPromptResult};

use crate::{
    Result,
    output::{
        Output,
        formatter::{MetadataDisplay, OutputFormatter, format_output},
    },
};

pub struct GetPromptFormatter;

impl OutputFormatter<GetPromptResult> for GetPromptFormatter {
    fn format_text(&self, output: &Output, result: &GetPromptResult) -> Result<()> {
        output.text(format!(
            "Prompt messages ({} item(s)):",
            result.messages.len()
        ))?;

        for (i, message) in result.messages.iter().enumerate() {
            output.text(format!("\n--- Message {} ---", i + 1))?;
            output.text(format!("Role: {:?}", message.role))?;

            match &message.content {
                schema::Content::Text(text_content) => {
                    MetadataDisplay::display_text_content(output, &text_content.text)?;
                }
                schema::Content::Image(image_content) => {
                    output.text(format!("Image content (MIME: {})", image_content.mime_type))?;
                    MetadataDisplay::display_binary_content(
                        output,
                        &image_content.data,
                        image_content.data.len(),
                    )?;
                }
                schema::Content::Audio(audio_content) => {
                    output.text(format!("Audio content (MIME: {})", audio_content.mime_type))?;
                    MetadataDisplay::display_binary_content(
                        output,
                        &audio_content.data,
                        audio_content.data.len(),
                    )?;
                }
                schema::Content::Resource(resource) => {
                    output.text("Embedded resource:")?;
                    match &resource.resource {
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
                schema::Content::ResourceLink(resource_link) => {
                    output.text("Resource link:")?;
                    MetadataDisplay::display_uri(output, &resource_link.uri)?;
                    output.text(format!("Name: {}", resource_link.name))?;
                    MetadataDisplay::display_title(output, &resource_link.title)?;
                    MetadataDisplay::display_description(output, &resource_link.description)?;
                    MetadataDisplay::display_mime_type(output, &resource_link.mime_type)?;
                }
            }
        }

        MetadataDisplay::display_description(output, &result.description)?;
        Ok(())
    }
}

pub fn get_prompt_result(output: &Output, result: &GetPromptResult) -> Result<()> {
    format_output(output, result, &GetPromptFormatter)
}
