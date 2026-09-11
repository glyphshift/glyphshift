use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const MAX_REGEX_RULES: usize = 64;
const MAX_OUTPUT_BYTES: usize = 16_384;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegexTranslationRule {
    pub pattern: String,
    pub replacement: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    Literal(String),
    Capture(usize),
    Translation(usize),
}

#[derive(Clone, Debug)]
struct CompiledRule {
    expression: Regex,
    parts: Vec<Part>,
}

#[derive(Clone, Debug, Default)]
pub struct RegexTranslationRules {
    rules: Vec<RegexTranslationRule>,
    compiled: Vec<CompiledRule>,
}

impl PartialEq for RegexTranslationRules {
    fn eq(&self, other: &Self) -> bool {
        self.rules == other.rules
    }
}
impl Eq for RegexTranslationRules {}

impl RegexTranslationRules {
    pub fn compile(rules: Vec<RegexTranslationRule>) -> Result<Self, String> {
        if rules.len() > MAX_REGEX_RULES {
            return Err("too_many_rules".into());
        }
        let mut compiled = Vec::new();
        for (index, rule) in rules.iter().enumerate() {
            let compile = || -> Result<CompiledRule, String> {
                if rule.pattern.is_empty()
                    || rule.pattern.len() > 1024
                    || rule.replacement.len() > 4096
                {
                    return Err("invalid_length".into());
                }
                let expression = RegexBuilder::new(&rule.pattern)
                    .size_limit(262_144)
                    .dfa_size_limit(262_144)
                    .build()
                    .map_err(|error| error.to_string())?;
                let parts = parse_replacement(&rule.replacement, &expression)?;
                Ok(CompiledRule { expression, parts })
            };
            let value = compile().map_err(|error| format!("{}: {error}", index + 1))?;
            compiled.push(value);
        }
        Ok(Self { rules, compiled })
    }

    pub fn rules(&self) -> &[RegexTranslationRule] {
        &self.rules
    }

    pub fn collection_sources(&self, source: &str) -> Option<Vec<String>> {
        let index = self.matching_rule_index(source)?;
        let rule = &self.compiled[index];
        let captures = rule.expression.captures(source)?;
        let mut sources = Vec::new();
        for part in &rule.parts {
            if let Part::Translation(group) = part {
                if let Some(value) = captures.get(*group) {
                    if !value.as_str().trim().is_empty() && !sources.iter().any(|s| s == value.as_str()) {
                        sources.push(value.as_str().to_owned());
                    }
                }
            }
        }
        Some(sources)
    }

    pub fn matching_rule_index(&self, source: &str) -> Option<usize> {
        if source.len() > MAX_OUTPUT_BYTES { return None; }
        self.rules.iter().zip(&self.compiled).position(|(rule, compiled)| rule.enabled && compiled.expression.is_match(source))
    }

    /// First matching rule wins. Missing dictionary translations keep the source;
    /// replacements never recursively feed back into the rules.
    pub fn replace(
        &self,
        source: &str,
        mut translate: impl FnMut(&str) -> Option<Arc<str>>,
    ) -> Option<Arc<str>> {
        if source.len() > MAX_OUTPUT_BYTES {
            return None;
        }
        for (rule, compiled) in self.rules.iter().zip(&self.compiled) {
            if !rule.enabled {
                continue;
            }
            let Some(captures) = compiled.expression.captures(source) else {
                continue;
            };
            let matched = captures.get(0)?;
            let mut output = String::from(&source[..matched.start()]);
            for part in &compiled.parts {
                match part {
                    Part::Literal(text) => output.push_str(text),
                    Part::Capture(index) => {
                        output.push_str(captures.get(*index).map_or("", |value| value.as_str()))
                    }
                    Part::Translation(index) => {
                        let original = captures.get(*index)?.as_str();
                        output.push_str(&translate(original)?);
                    }
                }
                if output.len() > MAX_OUTPUT_BYTES {
                    return None;
                }
            }
            output.push_str(&source[matched.end()..]);
            return (output.len() <= MAX_OUTPUT_BYTES).then(|| Arc::from(output));
        }
        None
    }
}

fn parse_replacement(template: &str, expression: &Regex) -> Result<Vec<Part>, String> {
    let mut rest = template;
    let mut parts = Vec::new();
    while !rest.is_empty() {
        if let Some(value) = rest.strip_prefix("{{TR}}") {
            capture_reference("$1", expression)?;
            parts.push(Part::Translation(1));
            rest = value;
        } else if let Some(value) = rest.strip_prefix("$$") {
            parts.push(Part::Literal("$".into()));
            rest = value;
        } else if rest.starts_with('$') {
            let (index, consumed) = capture_reference(rest, expression)?;
            parts.push(Part::Capture(index));
            rest = &rest[consumed..];
        } else {
            let length = rest.chars().next().ok_or("invalid_template")?.len_utf8();
            if let Some(Part::Literal(text)) = parts.last_mut() {
                text.push_str(&rest[..length]);
            } else {
                parts.push(Part::Literal(rest[..length].into()));
            }
            rest = &rest[length..];
        }
    }
    Ok(parts)
}

fn capture_reference(token: &str, expression: &Regex) -> Result<(usize, usize), String> {
    let rest = token.strip_prefix('$').ok_or("invalid_capture")?;
    let length = rest.bytes().take_while(u8::is_ascii_digit).count();
    if length == 0 {
        return Err("invalid_capture".into());
    }
    let index = rest[..length]
        .parse::<usize>()
        .map_err(|_| "invalid_capture")?;
    if index >= expression.captures_len() {
        return Err("unknown_capture".into());
    }
    Ok((index, length + 1))
}


#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegexRuleTestResult {
    pub matched: bool,
    pub captures: Vec<Option<String>>,
    pub output: String,
    pub missing_translation: bool,
}

/// A single unsaved rule with caller-supplied translation text; no dictionary access.
pub fn test_regex_rule(
    mut rule: RegexTranslationRule,
    source: &str,
    mock_translation: &str,
) -> Result<RegexRuleTestResult, String> {
    if source.len() > MAX_OUTPUT_BYTES || mock_translation.len() > MAX_OUTPUT_BYTES {
        return Err("invalid_length".into());
    }
    rule.enabled = true;
    let rules = RegexTranslationRules::compile(vec![rule])?;
    let captures = rules.compiled[0].expression.captures(source);
    let Some(captures) = captures else {
        return Ok(RegexRuleTestResult { matched: false, captures: vec![], output: source.into(), missing_translation: false });
    };
    let captures = captures.iter().map(|value| value.map(|value| value.as_str().to_owned())).collect();
    let mut missing_translation = false;
    let output = rules.replace(source, |_| {
        if mock_translation.is_empty() {
            missing_translation = true;
            None
        } else {
            Some(Arc::from(mock_translation))
        }
    });
    // An optional unmatched translation group also leaves the original intact.
    if output.is_none() && !missing_translation {
        let groups = rules.compiled[0].expression.captures(source).unwrap();
        missing_translation = rules.compiled[0].parts.iter().any(|part|
            matches!(part, Part::Translation(index) if groups.get(*index).is_none()));
        if !missing_translation { return Err("output_too_long".into()); }
    }
    Ok(RegexRuleTestResult { matched: true, captures, output: output.map_or_else(|| source.into(), |value| value.to_string()), missing_translation })
}
