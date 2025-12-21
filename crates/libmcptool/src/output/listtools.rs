use crate::{Result, output::Output};

fn toolschema(output: &Output, schema: &tmcp::schema::ToolSchema) -> Result<()> {
    if let Some(properties) = &schema.properties
        && !properties.is_empty()
    {
        // Sort properties to show required ones first
        let mut sorted_props: Vec<_> = properties.iter().collect();
        sorted_props.sort_by(|(a_name, _), (b_name, _)| {
            let a_required = schema.is_required(a_name);
            let b_required = schema.is_required(b_name);

            // Required fields come first
            b_required.cmp(&a_required).then(a_name.cmp(b_name))
        });

        for (name, prop_schema) in sorted_props {
            let is_required = schema.is_required(name);

            // Extract type from schema
            let type_str = if let Some(serde_json::Value::String(t)) = prop_schema.get("type") {
                t.to_string()
            } else if let Some(serde_json::Value::Array(types)) = prop_schema.get("type") {
                // Handle union types like ["string", "null"]
                types
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            } else {
                "unknown".to_string()
            };

            // Use kv() to display property name and type
            output.kv(name, &type_str)?;

            // Show schema details indented further
            let out = output.indent();

            // Show required marker on separate line if required
            if is_required {
                out.note("[required]")?;
            }

            // Make a mutable copy of the schema
            let mut schema_copy = prop_schema.clone();

            // Remove type since we already displayed it
            if let Some(obj) = schema_copy.as_object_mut() {
                obj.remove("type");

                // Extract and display description if it exists
                if let Some(serde_json::Value::String(desc)) = obj.remove("description") {
                    out.text(&desc)?;
                }

                // If there are remaining properties, display them as JSON
                if !obj.is_empty() {
                    let rendered_schema = serde_json::to_string_pretty(&schema_copy)?;
                    for line in rendered_schema.lines() {
                        out.text(line)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Display the list of tools in either JSON or formatted text
pub fn list_tools_result(
    output: &Output,
    tools_result: &tmcp::schema::ListToolsResult,
) -> Result<()> {
    if output.json {
        output.json_value(tools_result)?;
    } else if tools_result.tools.is_empty() {
        output.text("No tools.")?;
    } else {
        for tool in &tools_result.tools {
            output.h1(&tool.name)?;

            let out = output.indent();

            // Description
            if let Some(description) = &tool.description {
                for line in description.lines() {
                    out.text(line)?;
                }
            }

            // Annotations
            if let Some(annotations) = &tool.annotations {
                let out = out.indent();
                out.h2("Annotations")?;
                let out = out.indent();
                if let Some(title) = &annotations.title {
                    out.kv("title", title)?;
                }
            }

            // Input arguments
            if let Some(properties) = &tool.input_schema.properties
                && !properties.is_empty()
            {
                let out = out.indent();
                out.h2("Input")?;
                let out = out.indent();
                toolschema(&out, &tool.input_schema)?;
            }

            // Output schema
            if let Some(output_schema) = &tool.output_schema
                && let Some(properties) = &output_schema.properties
                && !properties.is_empty()
            {
                let out = out.indent();
                out.h2("Output")?;
                let out = out.indent();
                toolschema(&out, output_schema)?;
            }

            output.text("")?; // Extra blank line between tools
        }
    }
    Ok(())
}
