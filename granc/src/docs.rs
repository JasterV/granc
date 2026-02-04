// granc/src/docs.rs
use crate::formatter::FormattedString;
use colored::control::set_override;
use granc_core::prost_reflect::{EnumDescriptor, Kind, MessageDescriptor, ServiceDescriptor};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

pub struct DocsGenerator {
    output_dir: PathBuf,
    visited: HashSet<String>,
}

impl DocsGenerator {
    pub fn new(output_dir: PathBuf) -> Self {
        Self {
            output_dir,
            visited: HashSet::new(),
        }
    }

    /// Entry point for documentation generation.
    pub fn generate(&mut self, service: ServiceDescriptor) -> std::io::Result<()> {
        // Force colored output OFF so we get plain text for the markdown files
        set_override(false);

        if !self.output_dir.exists() {
            fs::create_dir_all(&self.output_dir)?;
        }

        // 1. Generate the Service page and recursively all dependencies
        self.generate_service(&service)?;

        // 2. Generate the Index (Table of Contents)
        self.generate_index(&service)?;

        // Restore colored output for the CLI
        set_override(true);
        Ok(())
    }

    fn generate_index(&self, service: &ServiceDescriptor) -> std::io::Result<()> {
        let path = self.output_dir.join("index.md");
        let mut out = String::new();

        out.push_str(&format!("# Documentation: `{}`\n\n", service.name()));

        out.push_str("## Entry Point\n\n");
        out.push_str(&format!(
            "- [**Service Definition: {}**]({}.md)\n",
            service.name(),
            service.full_name()
        ));

        out.push_str("\n## Messages & Enums\n\n");
        let mut types: Vec<_> = self.visited.iter().collect();
        types.sort();

        if types.is_empty() {
            out.push_str("*None*\n");
        } else {
            for name in types {
                out.push_str(&format!("- [{}]({}.md)\n", name, name));
            }
        }

        fs::write(path, out)?;
        println!("Generated: index.md");
        Ok(())
    }

    fn generate_service(&mut self, service: &ServiceDescriptor) -> std::io::Result<()> {
        let filename = format!("{}.md", service.full_name());
        let path = self.output_dir.join(&filename);

        let mut out = String::new();
        out.push_str(&format!("# Service: `{}`\n\n", service.name()));

        // 1. Protobuf Definition
        out.push_str("## Definition\n\n```protobuf\n");
        out.push_str(&FormattedString::from(service.clone()).0);
        out.push_str("\n```\n\n");

        // 2. Methods List
        out.push_str("## Methods\n\n");
        for method in service.methods() {
            out.push_str(&format!("### `{}`\n\n", method.name()));

            let input = method.input();
            let output = method.output();

            out.push_str(&format!(
                "- Request: [{}]({}.md)\n",
                input.full_name(),
                input.full_name()
            ));
            out.push_str(&format!(
                "- Response: [{}]({}.md)\n",
                output.full_name(),
                output.full_name()
            ));
            out.push('\n');

            // Queue recursion for dependencies
            self.queue_message(input);
            self.queue_message(output);
        }

        fs::write(path, out)?;
        println!("Generated: {}", filename);
        Ok(())
    }

    fn queue_message(&mut self, message: MessageDescriptor) {
        let name = message.full_name().to_string();
        if self.visited.contains(&name) {
            return;
        }
        self.visited.insert(name);

        if let Err(e) = self.generate_message(message) {
            eprintln!("Failed to generate docs for message: {}", e);
        }
    }

    fn generate_message(&mut self, message: MessageDescriptor) -> std::io::Result<()> {
        let filename = format!("{}.md", message.full_name());
        let path = self.output_dir.join(&filename);

        let mut out = String::new();
        out.push_str(&format!("# Message: `{}`\n\n", message.name()));

        // Definition
        out.push_str("## Definition\n\n```protobuf\n");
        out.push_str(&FormattedString::from(message.clone()).0);
        out.push_str("\n```\n\n");

        // Dependencies
        out.push_str("## Dependencies\n\n");
        let mut has_deps = false;

        for field in message.fields() {
            match field.kind() {
                Kind::Message(m) => {
                    has_deps = true;
                    out.push_str(&format!(
                        "- Field `{}`: [{}]({}.md)\n",
                        field.name(),
                        m.full_name(),
                        m.full_name()
                    ));
                    self.queue_message(m);
                }
                Kind::Enum(e) => {
                    has_deps = true;
                    out.push_str(&format!(
                        "- Field `{}`: [{}]({}.md)\n",
                        field.name(),
                        e.full_name(),
                        e.full_name()
                    ));
                    self.queue_enum(e);
                }
                _ => {}
            }
        }

        if !has_deps {
            out.push_str("*None*\n");
        }

        fs::write(path, out)?;
        println!("Generated: {}", filename);
        Ok(())
    }

    fn queue_enum(&mut self, enum_desc: EnumDescriptor) {
        let name = enum_desc.full_name().to_string();
        if self.visited.contains(&name) {
            return;
        }
        self.visited.insert(name);

        if let Err(e) = self.generate_enum(enum_desc) {
            eprintln!("Failed to generate docs for enum: {}", e);
        }
    }

    fn generate_enum(&mut self, enum_desc: EnumDescriptor) -> std::io::Result<()> {
        let filename = format!("{}.md", enum_desc.full_name());
        let path = self.output_dir.join(&filename);

        let mut out = String::new();
        out.push_str(&format!("# Enum: `{}`\n\n", enum_desc.name()));

        out.push_str("## Definition\n\n```protobuf\n");
        out.push_str(&FormattedString::from(enum_desc).0);
        out.push_str("\n```\n");

        fs::write(path, out)?;
        println!("Generated: {}", filename);
        Ok(())
    }
}
