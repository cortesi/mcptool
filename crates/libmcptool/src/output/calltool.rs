//! Call tool result display formatting.

use tmcp::schema::{Annotations, CallToolResult, Content, ResourceContents, Role};

use crate::{Result, output::Output};

/// Display the result of calling a tool in either JSON or formatted text.
pub fn call_tool_result(output: &Output, result: &CallToolResult) -> Result<()> {
    if output.json {
        output.json_value(result)?;
    } else {
        output.h1("Tool Result")?;

        let out = output.indent();

        // Show error status if this is an error result
        if let Some(is_error) = result.is_error {
            if is_error {
                out.trace_error("Tool execution resulted in an error")?;
            } else {
                out.trace_success("Tool executed successfully")?;
            }
        } else {
            out.trace_success("Tool executed successfully")?;
        }

        // Display content
        if !result.content.is_empty() {
            let out = out.indent();
            out.h2("Content")?;
            let out = out.indent();

            for (index, content) in result.content.iter().enumerate() {
                if result.content.len() > 1 {
                    out.h3(format!("Content {}", index + 1))?;
                    let out = out.indent();
                    display_content(&out, content)?;
                } else {
                    display_content(&out, content)?;
                }
            }
        }

        // Display structured content if present
        if let Some(structured) = &result.structured_content {
            let out = out.indent();
            out.h2("Structured Content")?;
            let out = out.indent();

            let structured_str = serde_json::to_string_pretty(structured)?;
            for line in structured_str.lines() {
                out.text(line)?;
            }
        }
    }
    Ok(())
}

/// Displays a single content item.
fn display_content(output: &Output, content: &Content) -> Result<()> {
    match content {
        Content::Text(text_content) => {
            output.kv("Type", "Text")?;
            let out = output.indent();

            // Display text content, handling multi-line text properly
            for line in text_content.text.lines() {
                out.text(line)?;
            }

            // Show annotations if present
            if let Some(annotations) = &text_content.annotations {
                display_annotations(&out, annotations)?;
            }
        }
        Content::Image(image_content) => {
            output.kv("Type", "Image")?;
            let out = output.indent();
            out.kv("MIME Type", &image_content.mime_type)?;
            out.kv("Data Length", format!("{} bytes", image_content.data.len()))?;

            // Show annotations if present
            if let Some(annotations) = &image_content.annotations {
                display_annotations(&out, annotations)?;
            }
        }
        Content::Audio(audio_content) => {
            output.kv("Type", "Audio")?;
            let out = output.indent();
            out.kv("MIME Type", &audio_content.mime_type)?;
            out.kv("Data Length", format!("{} bytes", audio_content.data.len()))?;

            // Show annotations if present
            if let Some(annotations) = &audio_content.annotations {
                display_annotations(&out, annotations)?;
            }
        }
        Content::Resource(resource) => {
            output.kv("Type", "Embedded Resource")?;
            let out = output.indent();
            display_resource_contents(&out, &resource.resource)?;

            // Show annotations if present
            if let Some(annotations) = &resource.annotations {
                display_annotations(&out, annotations)?;
            }
        }
        Content::ResourceLink(resource_link) => {
            output.kv("Type", "Resource Link")?;
            let out = output.indent();
            out.kv("Name", &resource_link.name)?;
            out.kv("URI", &resource_link.uri)?;

            if let Some(title) = &resource_link.title {
                out.kv("Title", title)?;
            }

            if let Some(description) = &resource_link.description {
                out.kv("Description", description)?;
            }

            if let Some(mime_type) = &resource_link.mime_type {
                out.kv("MIME Type", mime_type)?;
            }

            if let Some(size) = resource_link.size {
                out.kv("Size", format!("{} bytes", size))?;
            }

            // Show annotations if present
            if let Some(annotations) = &resource_link.annotations {
                display_annotations(&out, annotations)?;
            }
        }
    }
    Ok(())
}

/// Displays resource contents.
fn display_resource_contents(output: &Output, contents: &ResourceContents) -> Result<()> {
    match contents {
        ResourceContents::Text(text_contents) => {
            output.kv("Resource Type", "Text")?;
            output.kv("URI", &text_contents.uri)?;

            if let Some(mime_type) = &text_contents.mime_type {
                output.kv("MIME Type", mime_type)?;
            }

            let out = output.indent();
            out.text("Content:")?;
            let out = out.indent();

            for line in text_contents.text.lines() {
                out.text(line)?;
            }
        }
        ResourceContents::Blob(blob_contents) => {
            output.kv("Resource Type", "Blob")?;
            output.kv("URI", &blob_contents.uri)?;

            if let Some(mime_type) = &blob_contents.mime_type {
                output.kv("MIME Type", mime_type)?;
            }

            output.kv("Data Length", format!("{} bytes", blob_contents.blob.len()))?;
        }
    }
    Ok(())
}

/// Displays content annotations.
fn display_annotations(output: &Output, annotations: &Annotations) -> Result<()> {
    output.h3("Annotations")?;
    let out = output.indent();

    if let Some(audience) = &annotations.audience {
        let audience_str = audience
            .iter()
            .map(|role| match role {
                Role::User => "User",
                Role::Assistant => "Assistant",
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.kv("Audience", &audience_str)?;
    }

    if let Some(priority) = annotations.priority {
        out.kv("Priority", priority.to_string())?;
    }

    if let Some(last_modified) = &annotations.last_modified {
        out.kv("Last Modified", last_modified)?;
    }

    Ok(())
}
