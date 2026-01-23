use std::collections::HashSet;

use colored::*;
use granc_core::prost_reflect::{
    EnumDescriptor, Kind, MessageDescriptor, MethodDescriptor, ServiceDescriptor,
};
use tonic::Status;

/// A wrapper struct for a formatted, colored string.
///
/// Implements `Display` so it can be printed directly.
pub struct FormattedString(pub String);

/// A wrapper to indicate we want to print a message AND all its dependencies recursively.
pub struct ExpandedMessage(pub MessageDescriptor);

impl std::fmt::Display for FormattedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "")?;
        writeln!(f, "{}", self.0)?;
        Ok(())
    }
}

impl From<serde_json::Value> for FormattedString {
    fn from(value: serde_json::Value) -> Self {
        FormattedString(serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()))
    }
}

impl From<Status> for FormattedString {
    fn from(status: Status) -> Self {
        FormattedString(format!(
            "{} code={:?} message={:?}",
            "gRPC Failed:".red().bold(),
            status.code(),
            status.message()
        ))
    }
}

impl From<ServiceDescriptor> for FormattedString {
    fn from(service: ServiceDescriptor) -> Self {
        let mut out = String::new();
        out.push_str(&format!(
            "{} {} {{\n",
            "service".cyan(),
            service.name().green()
        ));

        for method in service.methods() {
            out.push_str("  ");
            // Reuse the From<MethodDescriptor> implementation
            let method_fmt = FormattedString::from(method);
            out.push_str(&method_fmt.0);
            out.push_str("\n\n");
        }
        out.push_str("}");
        FormattedString(out)
    }
}

impl From<MethodDescriptor> for FormattedString {
    fn from(method: MethodDescriptor) -> Self {
        let input_stream = if method.is_client_streaming() {
            format!("{} ", "stream".cyan())
        } else {
            "".to_string()
        };
        let output_stream = if method.is_server_streaming() {
            format!("{} ", "stream".cyan())
        } else {
            "".to_string()
        };

        FormattedString(format!(
            "{} {}({}{}) {} ({}{});",
            "rpc".cyan(),
            method.name().green(),
            input_stream,
            method.input().full_name().yellow(),
            "returns".cyan(),
            output_stream,
            method.output().full_name().yellow()
        ))
    }
}

impl From<MessageDescriptor> for FormattedString {
    fn from(message: MessageDescriptor) -> Self {
        let mut out = String::new();
        out.push_str(&format!(
            "{} {} {{\n",
            "message".cyan(),
            message.name().green()
        ));

        for field in message.fields() {
            let label = if field.is_map() {
                "".to_string()
            } else if field.is_list() {
                format!("{} ", "repeated".cyan())
            } else {
                "".to_string()
            };

            let type_name = match field.kind() {
                Kind::Double => "double".yellow(),
                Kind::Float => "float".yellow(),
                Kind::Int32 => "int32".yellow(),
                Kind::Int64 => "int64".yellow(),
                Kind::Uint32 => "uint32".yellow(),
                Kind::Uint64 => "uint64".yellow(),
                Kind::Sint32 => "sint32".yellow(),
                Kind::Sint64 => "sint64".yellow(),
                Kind::Fixed32 => "fixed32".yellow(),
                Kind::Fixed64 => "fixed64".yellow(),
                Kind::Sfixed32 => "sfixed32".yellow(),
                Kind::Sfixed64 => "sfixed64".yellow(),
                Kind::Bool => "bool".yellow(),
                Kind::String => "string".yellow(),
                Kind::Bytes => "bytes".yellow(),
                Kind::Message(m) => m.full_name().yellow(),
                Kind::Enum(e) => e.full_name().yellow(),
            };

            if field.is_map() {
                out.push_str(&format!(
                    "  // map entry: {} {} = {};\n",
                    type_name,
                    field.name(),
                    field.number()
                ));
            } else {
                out.push_str(&format!(
                    "  {}{}{} {} = {};\n",
                    label,
                    type_name,
                    " ".normal(), // Reset color
                    field.name(),
                    field.number()
                ));
            }
        }
        out.push_str("}");
        FormattedString(out)
    }
}

impl From<EnumDescriptor> for FormattedString {
    fn from(enum_desc: EnumDescriptor) -> Self {
        let mut out = String::new();
        out.push_str(&format!(
            "{} {} {{\n",
            "enum".cyan(),
            enum_desc.name().green()
        ));

        for val in enum_desc.values() {
            out.push_str(&format!(
                "  {} = {};\n",
                val.name(),
                val.number().to_string().purple()
            ));
        }
        out.push_str("}");

        FormattedString(out)
    }
}

// Logic to traverse and print the message + dependencies
impl From<ExpandedMessage> for FormattedString {
    fn from(wrapper: ExpandedMessage) -> Self {
        let root = wrapper.0;
        let mut out = String::new();
        let mut visited = HashSet::new();
        let mut stack = vec![Kind::Message(root.clone())];

        // First pass: Print root
        out.push_str(&FormattedString::from(root.clone()).0);
        out.push_str("\n");
        visited.insert(root.full_name().to_string());

        // Recursive pass
        // We iterate through the stack of things to visit.
        // For every message we visit, we scan its fields for more types (Messages/Enums)
        // and add them to the stack if not visited.
        // We append the printed output to `out`.
        let mut i = 0;
        while i < stack.len() {
            let current = stack[i].clone();
            i += 1; // move "pointer", we simulate queue behavior in a vec for simplicity or just DFS order

            match current {
                Kind::Message(m) => {
                    // For this message, find all sub-dependencies
                    for field in m.fields() {
                        let kind = field.kind();
                        match kind {
                            Kind::Message(sub_m) => {
                                if !visited.contains(sub_m.full_name()) {
                                    visited.insert(sub_m.full_name().to_string());
                                    stack.push(Kind::Message(sub_m.clone()));

                                    out.push_str("\n");
                                    out.push_str(&FormattedString::from(sub_m).0);
                                    out.push_str("\n");
                                }
                            }
                            Kind::Enum(sub_e) => {
                                if !visited.contains(sub_e.full_name()) {
                                    visited.insert(sub_e.full_name().to_string());
                                    stack.push(Kind::Enum(sub_e.clone()));

                                    out.push_str("\n");
                                    out.push_str(&FormattedString::from(sub_e).0);
                                    out.push_str("\n");
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Kind::Enum(_) => {
                    // Enums don't have further dependencies
                }
                _ => {}
            }
        }

        FormattedString(out)
    }
}
