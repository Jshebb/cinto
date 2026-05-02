use std::{error::Error, fmt, ops::Range, path::Path};

pub const VERSION: &str = "1.0-draft";

pub const DEFAULT_REQUIRED_SLOTS: &[&str] = &["FINAL_RESPONSE"];
pub const DEFAULT_FILE_PATH_SLOTS: &[&str] = &["RELEVANT_FILES"];

#[derive(Debug, Clone)]
pub struct ValidationConfig<'a> {
    pub workspace: Option<&'a Path>,
    pub required_slots: &'a [&'a str],
    pub file_path_slots: &'a [&'a str],
}

impl Default for ValidationConfig<'_> {
    fn default() -> Self {
        Self {
            workspace: None,
            required_slots: DEFAULT_REQUIRED_SLOTS,
            file_path_slots: DEFAULT_FILE_PATH_SLOTS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotStatus {
    Valid,
    Warning,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotOutcome {
    pub slot: String,
    pub status: SlotStatus,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub outcomes: Vec<SlotOutcome>,
}

impl ValidationReport {
    pub fn is_executable(&self) -> bool {
        !self
            .outcomes
            .iter()
            .any(|outcome| outcome.status == SlotStatus::Invalid)
    }

    pub fn invalid(&self) -> impl Iterator<Item = &SlotOutcome> {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.status == SlotStatus::Invalid)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &SlotOutcome> {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.status == SlotStatus::Warning)
    }
}

pub fn validate(trace: &Trace, config: &ValidationConfig<'_>) -> ValidationReport {
    let mut outcomes = Vec::new();

    for required in config.required_slots {
        match trace.get(required) {
            None => outcomes.push(SlotOutcome {
                slot: (*required).to_string(),
                status: SlotStatus::Invalid,
                message: format!(
                    "Required slot <{required}> is missing. Emit it with non-empty content."
                ),
            }),
            Some(slot) if slot.content.trim().is_empty() => outcomes.push(SlotOutcome {
                slot: (*required).to_string(),
                status: SlotStatus::Invalid,
                message: format!(
                    "Required slot <{required}> is present but empty. Provide concrete content."
                ),
            }),
            Some(_) => {}
        }
    }

    if let Some(workspace) = config.workspace {
        for slot_name in config.file_path_slots {
            for slot in trace.get_all(slot_name) {
                let mut missing = Vec::new();
                for path in extract_bullet_paths(&slot.content) {
                    if !workspace.join(&path).exists() {
                        missing.push(path);
                    }
                }
                if !missing.is_empty() {
                    outcomes.push(SlotOutcome {
                        slot: (*slot_name).to_string(),
                        status: SlotStatus::Invalid,
                        message: format!(
                            "Path(s) listed in <{slot_name}> do not exist in the workspace: {}. Use the read/list tools to confirm before listing files.",
                            missing.join(", ")
                        ),
                    });
                }
            }
        }
    }

    ValidationReport { outcomes }
}

pub fn build_retry_message(report: &ValidationReport) -> String {
    let mut buf = String::new();
    buf.push_str("<RETRY_REASON>\n");
    buf.push_str(
        "The previous response failed CRP validation. Produce a new complete CRP trace addressing the feedback below.\n",
    );
    buf.push_str("</RETRY_REASON>\n");

    for outcome in &report.outcomes {
        if outcome.status == SlotStatus::Valid {
            continue;
        }
        let status = match outcome.status {
            SlotStatus::Invalid => "INVALID",
            SlotStatus::Warning => "WARNING",
            SlotStatus::Valid => continue,
        };
        buf.push('\n');
        buf.push_str(&format!(
            "<SLOT_FEEDBACK slot=\"{}\" status=\"{}\">\n{}\n</SLOT_FEEDBACK>\n",
            outcome.slot, status, outcome.message
        ));
    }

    buf
}

pub fn build_parse_retry_message(error: &ParseError) -> String {
    format!(
        "<RETRY_REASON>\nThe previous response could not be parsed as a CRP trace ({error}). Re-emit the entire answer using uppercase CRP slot tags such as <FINAL_RESPONSE>...</FINAL_RESPONSE> and do not include prose outside slots.\n</RETRY_REASON>\n"
    )
}

fn extract_bullet_paths(content: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let stripped = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("• "))
            .unwrap_or(trimmed);
        let candidate = stripped.split_whitespace().next().unwrap_or("").trim();
        if candidate.is_empty() {
            continue;
        }
        paths.push(candidate.trim_end_matches(|c: char| matches!(c, ',' | ';')).to_string());
    }
    paths
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub slots: Vec<Slot>,
}

impl Trace {
    pub fn get(&self, name: &str) -> Option<&Slot> {
        self.slots.iter().find(|slot| slot.name == name)
    }

    pub fn get_all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Slot> + 'a {
        self.slots.iter().filter(move |slot| slot.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub content: String,
    pub byte_range: Range<usize>,
}

impl Slot {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name == name)
            .map(|attribute| attribute.value.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub position: usize,
    pub kind: ParseErrorKind,
}

impl ParseError {
    fn new(position: usize, kind: ParseErrorKind) -> Self {
        Self { position, kind }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    NoSlots,
    ExpectedOpeningTag,
    UnexpectedClosingTag,
    UnterminatedOpeningTag,
    InvalidSlotName(String),
    InvalidAttribute(String),
    MissingClosingTag { slot: String },
    MismatchedClosingTag { expected: String, found: String },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ParseErrorKind::NoSlots => write!(f, "CRP trace contains no slots"),
            ParseErrorKind::ExpectedOpeningTag => {
                write!(f, "expected CRP opening tag at byte {}", self.position)
            }
            ParseErrorKind::UnexpectedClosingTag => {
                write!(f, "unexpected CRP closing tag at byte {}", self.position)
            }
            ParseErrorKind::UnterminatedOpeningTag => {
                write!(f, "unterminated CRP opening tag at byte {}", self.position)
            }
            ParseErrorKind::InvalidSlotName(name) => {
                write!(
                    f,
                    "invalid CRP slot name `{name}` at byte {}",
                    self.position
                )
            }
            ParseErrorKind::InvalidAttribute(attribute) => {
                write!(
                    f,
                    "invalid CRP attribute `{attribute}` at byte {}",
                    self.position
                )
            }
            ParseErrorKind::MissingClosingTag { slot } => {
                write!(f, "missing CRP closing tag </{slot}>")
            }
            ParseErrorKind::MismatchedClosingTag { expected, found } => {
                write!(
                    f,
                    "mismatched CRP closing tag: expected </{expected}>, found </{found}>"
                )
            }
        }
    }
}

impl Error for ParseError {}

pub fn parse(input: &str) -> Result<Trace, ParseError> {
    let mut parser = Parser::new(input);
    let slots = parser.parse_slots()?;
    Ok(Trace { slots })
}

pub fn active_slot(input: &str) -> Option<String> {
    let mut position = 0;
    let mut active: Option<String> = None;

    loop {
        if let Some(name) = active.as_deref() {
            let close_tag = format!("</{name}>");
            let Some(relative_end) = input[position..].find(&close_tag) else {
                return active;
            };
            position += relative_end + close_tag.len();
            active = None;
            continue;
        }

        let Some(relative_start) = input[position..].find('<') else {
            return None;
        };
        let tag_start = position + relative_start;
        if input[tag_start..].starts_with("</") {
            position = tag_start + 2;
            continue;
        }

        let Some(relative_end) = input[tag_start..].find('>') else {
            return None;
        };
        let tag_end = tag_start + relative_end;
        let inner = &input[tag_start + 1..tag_end];
        let trimmed = inner.trim_end();
        if trimmed.is_empty()
            || inner
                .chars()
                .next()
                .is_some_and(|character| character.is_whitespace())
        {
            position = tag_end + 1;
            continue;
        }

        let name_end = trimmed
            .find(|character: char| character.is_ascii_whitespace())
            .unwrap_or(trimmed.len());
        let name = &trimmed[..name_end];
        if is_slot_name(name) {
            active = Some(name.to_string());
        }
        position = tag_end + 1;
    }
}

struct Parser<'a> {
    input: &'a str,
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, position: 0 }
    }

    fn parse_slots(&mut self) -> Result<Vec<Slot>, ParseError> {
        let mut slots = Vec::new();
        self.skip_whitespace();

        while self.position < self.input.len() {
            let slot = self.parse_slot()?;
            slots.push(slot);
            self.skip_whitespace();
        }

        if slots.is_empty() {
            return Err(ParseError::new(0, ParseErrorKind::NoSlots));
        }

        Ok(slots)
    }

    fn parse_slot(&mut self) -> Result<Slot, ParseError> {
        let start = self.position;
        let opening = self.parse_opening_tag()?;
        let close_tag = format!("</{}>", opening.name);
        let content_start = self.position;
        let Some(relative_content_end) = self.input[content_start..].find(&close_tag) else {
            if let Some((found, position)) = find_closing_tag(self.input, content_start) {
                return Err(ParseError::new(
                    position,
                    ParseErrorKind::MismatchedClosingTag {
                        expected: opening.name,
                        found,
                    },
                ));
            }

            return Err(ParseError::new(
                content_start,
                ParseErrorKind::MissingClosingTag { slot: opening.name },
            ));
        };

        let content_end = content_start + relative_content_end;
        let end = content_end + close_tag.len();
        self.position = end;

        Ok(Slot {
            name: opening.name,
            attributes: opening.attributes,
            content: self.input[content_start..content_end].trim().to_string(),
            byte_range: start..end,
        })
    }

    fn parse_opening_tag(&mut self) -> Result<OpeningTag, ParseError> {
        let start = self.position;
        if !self.input[start..].starts_with('<') {
            return Err(ParseError::new(start, ParseErrorKind::ExpectedOpeningTag));
        }
        if self.input[start..].starts_with("</") {
            return Err(ParseError::new(start, ParseErrorKind::UnexpectedClosingTag));
        }

        let Some(relative_end) = self.input[start..].find('>') else {
            return Err(ParseError::new(
                start,
                ParseErrorKind::UnterminatedOpeningTag,
            ));
        };
        let end = start + relative_end;
        let inner = &self.input[start + 1..end];
        let trimmed = inner.trim_end();
        if trimmed.is_empty() {
            return Err(ParseError::new(
                start + 1,
                ParseErrorKind::InvalidSlotName(String::new()),
            ));
        }
        if inner
            .chars()
            .next()
            .is_some_and(|character| character.is_whitespace())
        {
            return Err(ParseError::new(
                start + 1,
                ParseErrorKind::InvalidSlotName(trimmed.to_string()),
            ));
        }
        if trimmed.ends_with('/') {
            return Err(ParseError::new(
                start + 1,
                ParseErrorKind::InvalidSlotName(trimmed.to_string()),
            ));
        }

        let name_end = trimmed
            .find(|character: char| character.is_ascii_whitespace())
            .unwrap_or(trimmed.len());
        let name = &trimmed[..name_end];
        if !is_slot_name(name) {
            return Err(ParseError::new(
                start + 1,
                ParseErrorKind::InvalidSlotName(name.to_string()),
            ));
        }

        let attributes = parse_attributes(&trimmed[name_end..], start + 1 + name_end)?;
        self.position = end + 1;

        Ok(OpeningTag {
            name: name.to_string(),
            attributes,
        })
    }

    fn skip_whitespace(&mut self) {
        while let Some(character) = self.input[self.position..].chars().next() {
            if !character.is_whitespace() {
                break;
            }
            self.position += character.len_utf8();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpeningTag {
    name: String,
    attributes: Vec<Attribute>,
}

fn parse_attributes(input: &str, base_position: usize) -> Result<Vec<Attribute>, ParseError> {
    let mut attributes = Vec::new();
    let mut position = 0;

    while position < input.len() {
        position = skip_ascii_whitespace(input, position);
        if position >= input.len() {
            break;
        }

        let name_start = position;
        while position < input.len() {
            let byte = input.as_bytes()[position];
            if !is_attribute_name_byte(byte) {
                break;
            }
            position += 1;
        }
        if position == name_start {
            return Err(ParseError::new(
                base_position + position,
                ParseErrorKind::InvalidAttribute(input[position..].to_string()),
            ));
        }

        let name = &input[name_start..position];
        position = skip_ascii_whitespace(input, position);
        if input.as_bytes().get(position) != Some(&b'=') {
            return Err(ParseError::new(
                base_position + position,
                ParseErrorKind::InvalidAttribute(name.to_string()),
            ));
        }
        position += 1;
        position = skip_ascii_whitespace(input, position);

        let Some(quote) = input.as_bytes().get(position).copied() else {
            return Err(ParseError::new(
                base_position + position,
                ParseErrorKind::InvalidAttribute(name.to_string()),
            ));
        };
        if quote != b'"' && quote != b'\'' {
            return Err(ParseError::new(
                base_position + position,
                ParseErrorKind::InvalidAttribute(name.to_string()),
            ));
        }
        position += 1;

        let value_start = position;
        while input
            .as_bytes()
            .get(position)
            .is_some_and(|byte| *byte != quote)
        {
            position += 1;
        }
        if position >= input.len() {
            return Err(ParseError::new(
                base_position + value_start,
                ParseErrorKind::InvalidAttribute(name.to_string()),
            ));
        }

        attributes.push(Attribute {
            name: name.to_string(),
            value: input[value_start..position].to_string(),
        });
        position += 1;
    }

    Ok(attributes)
}

fn find_closing_tag(input: &str, start: usize) -> Option<(String, usize)> {
    let mut search_start = start;

    while let Some(relative_start) = input[search_start..].find("</") {
        let tag_start = search_start + relative_start;
        let name_start = tag_start + 2;
        let Some(relative_end) = input[name_start..].find('>') else {
            return None;
        };
        let name_end = name_start + relative_end;
        let name = &input[name_start..name_end];
        if is_slot_name(name) {
            return Some((name.to_string(), tag_start));
        }
        search_start = name_end + 1;
    }

    None
}

fn skip_ascii_whitespace(input: &str, mut position: usize) -> usize {
    while input
        .as_bytes()
        .get(position)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        position += 1;
    }
    position
}

fn is_slot_name(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !first.is_ascii_uppercase() {
        return false;
    }
    characters.all(|character| {
        character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
    })
}

fn is_attribute_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_supported_version() {
        assert_eq!(VERSION, "1.0-draft");
    }

    #[test]
    fn parses_sequential_slots_and_trims_content() {
        let trace = parse(
            r#"
<TASK_INTERPRETATION>
  Add a parser module.
</TASK_INTERPRETATION>

<FINAL_RESPONSE>
  Done.
</FINAL_RESPONSE>
"#,
        )
        .expect("valid CRP");

        assert_eq!(trace.slots.len(), 2);
        assert_eq!(trace.slots[0].name, "TASK_INTERPRETATION");
        assert_eq!(trace.slots[0].content, "Add a parser module.");
        assert_eq!(trace.slots[1].content, "Done.");
    }

    #[test]
    fn preserves_unknown_slots_and_attributes() {
        let trace = parse(
            r#"<SLOT_FEEDBACK slot="RELEVANT_FILES" status="INVALID">
src/missing.rs does not exist.
</SLOT_FEEDBACK>"#,
        )
        .expect("valid CRP");

        let slot = trace.get("SLOT_FEEDBACK").expect("slot");
        assert_eq!(slot.attribute("slot"), Some("RELEVANT_FILES"));
        assert_eq!(slot.attribute("status"), Some("INVALID"));
        assert_eq!(slot.content, "src/missing.rs does not exist.");
    }

    #[test]
    fn treats_content_as_opaque_until_matching_close_tag() {
        let trace = parse(
            r#"<FINAL_RESPONSE>
<div class="note">HTML is fine.</div>
let text = "<NOT_A_SLOT>";
</FINAL_RESPONSE>"#,
        )
        .expect("valid CRP");

        assert!(trace.slots[0].content.contains("</div>"));
        assert!(trace.slots[0].content.contains("<NOT_A_SLOT>"));
    }

    #[test]
    fn keeps_file_edit_subblocks_as_slot_content() {
        let trace = parse(
            r#"<FILE_EDITS>
<EDIT path="src/main.rs" mode="prepend_to_existing">
fn hello() {}
</EDIT>
</FILE_EDITS>"#,
        )
        .expect("valid CRP");

        let edits = trace.get("FILE_EDITS").expect("file edits");
        assert!(edits.content.contains("<EDIT path=\"src/main.rs\""));
        assert!(edits.content.contains("</EDIT>"));
    }

    #[test]
    fn preserves_diff_like_file_edits_as_slot_content() {
        let trace = parse(
            r#"<FILE_EDITS>
@@ src/main.rs prepend
fn hello() { println!("hi"); }
@@ src/lib.rs replace_function:greet
pub fn greet() -> String { "hello".into() }
</FILE_EDITS>"#,
        )
        .expect("valid CRP");

        let edits = trace.get("FILE_EDITS").expect("file edits");
        assert!(edits.content.contains("@@ src/main.rs prepend"));
        assert!(edits.content.contains("replace_function:greet"));
    }

    #[test]
    fn reports_active_slot_for_partial_traces() {
        assert_eq!(
            active_slot("<TASK_INTERPRETATION>\nReading..."),
            Some("TASK_INTERPRETATION".to_string())
        );
        assert_eq!(
            active_slot("<FINAL_RESPONSE>\n<NOT_A_SLOT>\nStill final."),
            Some("FINAL_RESPONSE".to_string())
        );
        assert_eq!(active_slot("<FINAL_RESPONSE>Done</FINAL_RESPONSE>"), None);
    }

    #[test]
    fn rejects_non_crp_text_outside_slots() {
        let error = parse("hello\n<FINAL_RESPONSE>Done</FINAL_RESPONSE>").expect_err("invalid");

        assert_eq!(error.kind, ParseErrorKind::ExpectedOpeningTag);
        assert_eq!(error.position, 0);
    }

    #[test]
    fn rejects_lowercase_slot_names() {
        let error = parse("<final>Done</final>").expect_err("invalid");

        assert_eq!(
            error.kind,
            ParseErrorKind::InvalidSlotName("final".to_string())
        );
    }

    #[test]
    fn rejects_leading_whitespace_in_opening_tag() {
        let error = parse("< FINAL_RESPONSE>Done</FINAL_RESPONSE>").expect_err("invalid");

        assert_eq!(
            error.kind,
            ParseErrorKind::InvalidSlotName(" FINAL_RESPONSE".to_string())
        );
    }

    #[test]
    fn reports_missing_closing_tag() {
        let error = parse("<FINAL_RESPONSE>Done").expect_err("invalid");

        assert_eq!(
            error.kind,
            ParseErrorKind::MissingClosingTag {
                slot: "FINAL_RESPONSE".to_string()
            }
        );
    }

    #[test]
    fn reports_mismatched_closing_tag() {
        let error = parse("<FINAL_RESPONSE>Done</TASK_INTERPRETATION>").expect_err("invalid");

        assert_eq!(
            error.kind,
            ParseErrorKind::MismatchedClosingTag {
                expected: "FINAL_RESPONSE".to_string(),
                found: "TASK_INTERPRETATION".to_string()
            }
        );
    }

    #[test]
    fn validate_flags_missing_required_final_response() {
        let trace = parse("<TASK_INTERPRETATION>Add hello.</TASK_INTERPRETATION>").expect("parse");
        let report = validate(&trace, &ValidationConfig::default());

        assert!(!report.is_executable());
        let invalid: Vec<_> = report.invalid().collect();
        assert_eq!(invalid.len(), 1);
        assert_eq!(invalid[0].slot, "FINAL_RESPONSE");
    }

    #[test]
    fn validate_flags_empty_required_slot() {
        let trace = parse("<FINAL_RESPONSE>   </FINAL_RESPONSE>").expect("parse");
        let report = validate(&trace, &ValidationConfig::default());

        assert!(!report.is_executable());
        assert_eq!(report.invalid().count(), 1);
    }

    #[test]
    fn validate_passes_when_required_slot_has_content() {
        let trace = parse("<FINAL_RESPONSE>Done.</FINAL_RESPONSE>").expect("parse");
        let report = validate(&trace, &ValidationConfig::default());

        assert!(report.is_executable());
    }

    #[test]
    fn validate_flags_missing_relevant_files_paths() {
        let workspace = std::env::temp_dir().join(format!(
            "cinto-crp-validate-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&workspace);
        std::fs::create_dir_all(&workspace).expect("create workspace");
        std::fs::write(workspace.join("real.rs"), "// real").expect("write real");

        let trace = parse(
            r#"<RELEVANT_FILES>
- real.rs
- ghost.rs
</RELEVANT_FILES>
<FINAL_RESPONSE>Done.</FINAL_RESPONSE>"#,
        )
        .expect("parse");

        let config = ValidationConfig {
            workspace: Some(&workspace),
            ..ValidationConfig::default()
        };
        let report = validate(&trace, &config);

        assert!(!report.is_executable());
        let invalid: Vec<_> = report.invalid().collect();
        assert_eq!(invalid.len(), 1);
        assert_eq!(invalid[0].slot, "RELEVANT_FILES");
        assert!(invalid[0].message.contains("ghost.rs"));
        assert!(!invalid[0].message.contains("real.rs"));

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn build_retry_message_emits_one_feedback_per_invalid_slot() {
        let trace = parse("<TASK_INTERPRETATION>x</TASK_INTERPRETATION>").expect("parse");
        let report = validate(&trace, &ValidationConfig::default());
        let retry = build_retry_message(&report);

        assert!(retry.contains("<RETRY_REASON>"));
        assert!(retry.contains("<SLOT_FEEDBACK slot=\"FINAL_RESPONSE\" status=\"INVALID\">"));
        assert!(retry.contains("</SLOT_FEEDBACK>"));
    }

    #[test]
    fn build_parse_retry_message_includes_error_detail() {
        let error = parse("hello").expect_err("invalid");
        let retry = build_parse_retry_message(&error);
        assert!(retry.contains("<RETRY_REASON>"));
        assert!(retry.contains("could not be parsed"));
    }
}
