use std::collections::HashMap;

use crate::parser::ast::*;
use crate::parser::{INTERNAL_OPERATORS, RsBlob, WithParsing};

pub struct NormalizedBlob {
    pub header: Vec<String>,
    pub sections: Vec<Section>,
    pub string_table: Vec<String>,
    pub import_types: Vec<String>,
    pub import_properties: Vec<String>,
    pub import_functions: Vec<String>,
}

pub struct Section {
    pub category: &'static str,
    pub key: String,
    pub lines: Vec<String>,
}

impl NormalizedBlob {
    pub fn from_blob(blob: &RsBlob) -> Self {
        let resolver = Resolver { blob };

        Self {
            header: resolver.header_lines(),
            sections: resolver.sections(),
            string_table: blob.string_table.clone(),
            import_types: blob
                .import_type_table
                .iter()
                .map(|t| format!("{} kind={}", resolver.name(&t.name), t.kind))
                .collect(),
            import_properties: blob
                .import_property_table
                .iter()
                .map(|p| {
                    format!(
                        "{}.{}: {}",
                        resolver.import_type(&p.scope_type),
                        resolver.name(&p.name),
                        resolver.import_type(&p.type_ref)
                    )
                })
                .collect(),
            import_functions: blob
                .import_function_table
                .iter()
                .map(|f| resolver.qualified(&f.scope_type, &f.name))
                .collect(),
        }
    }
}

impl std::fmt::Display for NormalizedBlob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "## header")?;
        for line in &self.header {
            writeln!(f, "{line}")?;
        }

        for section in &self.sections {
            writeln!(f, "\n## {} {}", section.category, section.key)?;
            for line in &section.lines {
                writeln!(f, "{line}")?;
            }
        }

        let tables = [
            ("string table", &self.string_table),
            ("import types", &self.import_types),
            ("import properties", &self.import_properties),
            ("import functions", &self.import_functions),
        ];

        for (title, entries) in tables {
            writeln!(f, "\n## {title} ({})", entries.len())?;
            for (index, entry) in entries.iter().enumerate() {
                writeln!(f, "{index}: {entry}")?;
            }
        }

        Ok(())
    }
}

#[derive(Default)]
struct SectionList {
    sections: Vec<Section>,
    occurrences: HashMap<(&'static str, String), usize>,
}

impl SectionList {
    fn push(&mut self, category: &'static str, key: String, lines: Vec<String>) {
        let occurrence = self
            .occurrences
            .entry((category, key.clone()))
            .and_modify(|count| *count += 1)
            .or_insert(1);

        let key = match *occurrence {
            1 => key,
            n => format!("{key}#{n}"),
        };

        self.sections.push(Section {
            category,
            key,
            lines,
        });
    }
}

const NO_IMPORT_TYPE: u32 = u32::MAX;

struct Resolver<'a> {
    blob: &'a RsBlob,
}

impl Resolver<'_> {
    fn name(&self, name: &CName) -> String {
        let index = name.string_index();

        match self.blob.string_table.get(index as usize) {
            Some(string) => string.clone(),
            None => format!("<string #{index}>"),
        }
    }

    fn import_type(&self, type_ref: &ImportTypeRef) -> String {
        match self.blob.import_type_table.get(type_ref.index as usize) {
            Some(import_type) => self.name(&import_type.name),
            None if type_ref.index == NO_IMPORT_TYPE => "<none>".to_owned(),
            None => format!("<import type #{}>", type_ref.index),
        }
    }

    fn qualified(&self, scope: &ImportTypeRef, name: &CName) -> String {
        match scope.index {
            NO_IMPORT_TYPE => self.name(name),
            _ => format!("{}::{}", self.import_type(scope), self.name(name)),
        }
    }

    fn import_function(&self, function_ref: &ImportFunctionRef) -> String {
        match self
            .blob
            .import_function_table
            .get(function_ref.table_index as usize)
        {
            Some(function) => self.qualified(&function.scope_type, &function.name),
            None => format!("<import function #{}>", function_ref.table_index),
        }
    }

    fn operand(&self, value: &OperandValue) -> String {
        match value {
            OperandValue::Bool(v) => v.to_string(),
            OperandValue::Unsigned8(v) => v.to_string(),
            OperandValue::Unsigned16(v) => v.to_string(),
            OperandValue::Unsigned32(v) => v.to_string(),
            OperandValue::Integer8(v) => v.to_string(),
            OperandValue::Integer16(v) => v.to_string(),
            OperandValue::Integer32(v) => v.to_string(),
            OperandValue::Float(v) => format!("{v:?}"),
            OperandValue::Name(name) => format!("'{}'", self.name(name)),
            OperandValue::String(string) => format!("{string:?}"),
            OperandValue::ImportFunction(function_ref) => self.import_function(function_ref),
            OperandValue::ImportFunctionInternal(operator) => INTERNAL_OPERATORS
                .get(operator.table_index)
                .map(|name| name.to_string())
                .unwrap_or_else(|| format!("<operator #{}>", operator.table_index)),
            OperandValue::ImportFunctionSelf => "<self>".to_owned(),
            OperandValue::ImportType(type_ref) => self.import_type(type_ref),
            OperandValue::ClassProp { name, type_ref } => {
                format!("{}.{}", self.import_type(type_ref), self.name(name))
            }
        }
    }

    fn header_lines(&self) -> Vec<String> {
        let blob = self.blob;

        vec![
            format!("format_version {}", blob.format_version),
            format!("build_platform {:?}", blob.build_platform),
            format!("build_version {:?}", blob.build_version),
            format!(
                "timestamp date={:#010x} time={:#010x}",
                blob.timestamp.date, blob.timestamp.time
            ),
            format!("build_config {:?}", blob.build_config),
        ]
    }

    fn sections(&self) -> Vec<Section> {
        let blob = self.blob;
        let mut sections = SectionList::default();

        for definition in &blob.enums {
            sections.push("enum", self.name(&definition.name), self.enum_lines(definition));
        }

        for definition in &blob.structs {
            sections.push(
                "struct",
                self.name(&definition.name),
                self.struct_lines(definition),
            );
        }

        for definition in &blob.classes {
            let class_name = self.name(&definition.name);
            sections.push("class", class_name.clone(), self.class_lines(definition));

            for function in &definition.functions {
                sections.push(
                    "class function",
                    format!("{class_name}::{}", self.name(&function.name)),
                    self.function_lines(function),
                );
            }
        }

        let function_groups = [
            ("global function", &blob.global_functions),
            ("@replaceMethod global", &blob.ext_replace_global_functions),
            ("@replaceMethod class", &blob.ext_replace_class_functions),
            ("@addMethod", &blob.ext_add_functions),
            ("@wrapMethod", &blob.ext_wrap_functions),
        ];

        for (category, functions) in function_groups {
            for function in functions.iter() {
                sections.push(
                    category,
                    self.owned_key(&function.override_class, &function.name),
                    self.function_lines(function),
                );
            }
        }

        for property in &blob.ext_add_properties {
            sections.push(
                "@addField",
                self.owned_key(&property.class_name, &property.name),
                vec![self.property_line(property)],
            );
        }

        sections.sections
    }

    fn owned_key(&self, owner: &CName, name: &CName) -> String {
        match self.name(owner).as_str() {
            "" => self.name(name),
            owner => format!("{owner}::{}", self.name(name)),
        }
    }

    fn enum_lines(&self, definition: &EnumDefinition) -> Vec<String> {
        definition
            .enumerators
            .iter()
            .map(|e| format!("enumerator {} = {}", self.name(&e.name), e.value))
            .collect()
    }

    fn struct_lines(&self, definition: &StructDefinition) -> Vec<String> {
        let mut lines = vec![format!("flags {:#x}", definition.flags)];
        lines.extend(definition.properties.iter().map(|p| self.property_line(p)));
        lines.extend(self.default_value_lines(&definition.default_values));

        lines
    }

    fn class_lines(&self, definition: &ClassDefinition) -> Vec<String> {
        let mut lines = vec![
            format!("parent {:?}", self.name(&definition.parent_name)),
            format!("machine {:?}", self.name(&definition.machine_name)),
            format!("is_state {}", definition.is_state),
            format!("flags {:#x}", definition.flags),
        ];
        lines.extend(definition.properties.iter().map(|p| self.property_line(p)));
        lines.extend(self.default_value_lines(&definition.default_values));

        lines
    }

    fn property_line(&self, property: &PropertyDefinition) -> String {
        format!(
            "property {}: {} flags={:#x} class={:?} binding={:?} hint={:?}",
            self.name(&property.name),
            self.name(&property.type_name),
            property.flags,
            self.name(&property.class_name),
            self.name(&property.binding),
            property.hint
        )
    }

    fn default_value_lines(&self, definitions: &[DefaultValueDefinition]) -> Vec<String> {
        let mut lines = Vec::new();

        for definition in definitions {
            lines.push(format!("default {}", self.name(&definition.name)));
            self.push_default_value(&definition.value, 1, &mut lines);
        }

        lines
    }

    fn push_default_value(&self, value: &DefaultValue, depth: usize, lines: &mut Vec<String>) {
        lines.push(format!(
            "{}{:?} = {:?}",
            "  ".repeat(depth),
            self.name(&value.name),
            value.value
        ));

        for sub_value in &value.sub_values {
            self.push_default_value(sub_value, depth + 1, lines);
        }
    }

    fn function_lines(&self, function: &FunctionDefinition) -> Vec<String> {
        let mut lines = vec![
            format!("override_class {:?}", self.name(&function.override_class)),
            format!("flags {:#x}", function.flags),
            match &function.return_type {
                Some(return_type) => format!("return {}", self.property_line(return_type)),
                None => "return <none>".to_owned(),
            },
        ];

        lines.extend(
            function
                .parameters
                .iter()
                .map(|p| format!("param {}", self.property_line(p))),
        );
        lines.extend(
            function
                .locals
                .iter()
                .map(|p| format!("local {}", self.property_line(p))),
        );
        lines.extend(self.bytecode_lines(function.bytecode.as_ref()));

        lines
    }

    fn bytecode_lines(&self, bytecode: &[u8]) -> Vec<String> {
        let Some(declared_runtime_size) = bytecode.first_chunk::<4>().map(|b| u32::from_le_bytes(*b))
        else {
            return vec![format!("bytecode <{} bytes, no header>", bytecode.len())];
        };

        let parsed = match ParsedFunctionBytecode::parse(bytecode) {
            Ok((_, parsed)) => parsed,
            Err(error) => return vec![format!("bytecode <decode error: {error:?}>")],
        };

        let mut lines = vec![format!("bytecode runtime_size={declared_runtime_size}")];
        lines.extend(parsed.instructions.iter().map(|i| self.instruction_line(i)));

        lines
    }

    fn instruction_line(&self, instruction: &Instruction) -> String {
        let operands: Vec<String> = instruction
            .op
            .operands()
            .iter()
            .map(|(name, value)| format!("{name}={}", self.operand(value)))
            .collect();

        match operands.is_empty() {
            true => format!("  {}", instruction.mnemo()),
            false => format!("  {} {}", instruction.mnemo(), operands.join(" ")),
        }
    }
}
