use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display, Write};

use super::normalize::{NormalizedBlob, Section};

const MAX_SECTION_DIFFS: usize = 10;

pub struct BlobDiff {
    header: Option<String>,
    order: Vec<String>,
    missing: Vec<String>,
    extra: Vec<String>,
    changed: Vec<(String, String)>,
    tables: Vec<String>,
}

impl BlobDiff {
    pub fn new(expected: &NormalizedBlob, actual: &NormalizedBlob) -> Self {
        let header = (expected.header != actual.header)
            .then(|| unified_diff(&expected.header, &actual.header));

        let expected_sections = index_sections(&expected.sections);
        let actual_sections = index_sections(&actual.sections);

        let missing = section_names(&expected.sections, |s| !actual_sections.contains_key(&section_id(s)));
        let extra = section_names(&actual.sections, |s| !expected_sections.contains_key(&section_id(s)));

        let changed = expected
            .sections
            .iter()
            .filter_map(|section| {
                let other = actual_sections.get(&section_id(section))?;
                (section.lines != other.lines).then(|| {
                    (
                        section_name(section),
                        unified_diff(&section.lines, &other.lines),
                    )
                })
            })
            .collect();

        let tables = [
            compare_table("string table", &expected.string_table, &actual.string_table),
            compare_table("import types", &expected.import_types, &actual.import_types),
            compare_table(
                "import properties",
                &expected.import_properties,
                &actual.import_properties,
            ),
            compare_table(
                "import functions",
                &expected.import_functions,
                &actual.import_functions,
            ),
        ]
        .into_iter()
        .flatten()
        .collect();

        Self {
            header,
            order: compare_section_order(&expected.sections, &actual.sections),
            missing,
            extra,
            changed,
            tables,
        }
    }

    pub fn is_semantically_equal(&self) -> bool {
        self.header.is_none()
            && self.order.is_empty()
            && self.missing.is_empty()
            && self.extra.is_empty()
            && self.changed.is_empty()
    }

    pub fn is_equal(&self) -> bool {
        self.is_semantically_equal() && self.tables.is_empty()
    }
}

impl Display for BlobDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_equal() {
            return writeln!(f, "✔️ normalized blobs are equal");
        }

        if let Some(header) = &self.header {
            writeln!(f, "## header\n{header}")?;
        }

        write_list(f, "definition order", &self.order)?;
        write_list(f, "missing in actual", &self.missing)?;
        write_list(f, "extra in actual", &self.extra)?;

        if !self.changed.is_empty() {
            writeln!(f, "## changed definitions ({})", self.changed.len())?;

            for (name, diff) in self.changed.iter().take(MAX_SECTION_DIFFS) {
                writeln!(f, "### {name}\n{diff}")?;
            }

            for (name, _) in self.changed.iter().skip(MAX_SECTION_DIFFS) {
                writeln!(f, "- {name}")?;
            }
        }

        write_list(f, "tables", &self.tables)
    }
}

fn write_list(f: &mut fmt::Formatter<'_>, title: &str, items: &[String]) -> fmt::Result {
    if items.is_empty() {
        return Ok(());
    }

    writeln!(f, "## {title} ({})", items.len())?;
    for item in items {
        writeln!(f, "- {item}")?;
    }

    Ok(())
}

type SectionId<'a> = (&'static str, &'a str);

fn section_id(section: &Section) -> SectionId<'_> {
    (section.category, &section.key)
}

fn section_name(section: &Section) -> String {
    format!("{} {}", section.category, section.key)
}

fn index_sections(sections: &[Section]) -> HashMap<SectionId<'_>, &Section> {
    sections.iter().map(|s| (section_id(s), s)).collect()
}

fn section_names(sections: &[Section], predicate: impl Fn(&Section) -> bool) -> Vec<String> {
    sections
        .iter()
        .filter(|s| predicate(s))
        .map(section_name)
        .collect()
}

fn compare_section_order(expected: &[Section], actual: &[Section]) -> Vec<String> {
    let categories: Vec<&'static str> = expected.iter().map(|s| s.category).fold(
        Vec::new(),
        |mut categories, category| {
            if !categories.contains(&category) {
                categories.push(category);
            }
            categories
        },
    );

    categories
        .into_iter()
        .filter_map(|category| {
            let expected_keys = keys_in_category(expected, category);
            let actual_keys = keys_in_category(actual, category);

            let shared: HashSet<&str> = expected_keys
                .iter()
                .copied()
                .filter(|key| actual_keys.contains(key))
                .collect();

            let expected_shared = expected_keys.iter().filter(|key| shared.contains(*key));
            let actual_shared = actual_keys.iter().filter(|key| shared.contains(*key));

            expected_shared
                .zip(actual_shared)
                .enumerate()
                .find(|(_, (a, b))| a != b)
                .map(|(index, (a, b))| {
                    format!("{category}: diverges at position {index}, expected {a}, actual {b}")
                })
        })
        .collect()
}

fn keys_in_category<'a>(sections: &'a [Section], category: &str) -> Vec<&'a str> {
    sections
        .iter()
        .filter(|s| s.category == category)
        .map(|s| s.key.as_str())
        .collect()
}

fn compare_table(label: &str, expected: &[String], actual: &[String]) -> Option<String> {
    if expected == actual {
        return None;
    }

    let expected_set: HashSet<&String> = expected.iter().collect();
    let actual_set: HashSet<&String> = actual.iter().collect();

    let missing: Vec<&String> = expected.iter().filter(|e| !actual_set.contains(e)).collect();
    let extra: Vec<&String> = actual.iter().filter(|e| !expected_set.contains(e)).collect();

    if missing.is_empty() && extra.is_empty() {
        let index = expected
            .iter()
            .zip(actual)
            .position(|(a, b)| a != b)
            .unwrap_or(expected.len().min(actual.len()));

        return Some(format!(
            "{label}: same entries, order diverges at index {index}, expected {:?}, actual {:?}",
            expected.get(index),
            actual.get(index)
        ));
    }

    let mut message = format!(
        "{label}: {} missing, {} extra (expected {} entries, actual {})",
        missing.len(),
        extra.len(),
        expected.len(),
        actual.len()
    );

    if let Some(first) = missing.first() {
        write!(message, ", first missing {first:?}").unwrap();
    }
    if let Some(first) = extra.first() {
        write!(message, ", first extra {first:?}").unwrap();
    }

    Some(message)
}

fn unified_diff(expected: &[String], actual: &[String]) -> String {
    let expected = expected.join("\n") + "\n";
    let actual = actual.join("\n") + "\n";

    similar::TextDiff::from_lines(&expected, &actual)
        .unified_diff()
        .context_radius(3)
        .header("expected", "actual")
        .to_string()
}
