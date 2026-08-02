use serde::Serialize;
use std::collections::BTreeMap;

pub(crate) const COMMAND_ERROR_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, Serialize, PartialEq)]
#[serde(untagged)]
pub(crate) enum CommandErrorArg {
    Text(Box<str>),
    Number(u64),
    Boolean(bool),
}

impl From<&str> for CommandErrorArg {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

impl From<String> for CommandErrorArg {
    fn from(value: String) -> Self {
        Self::Text(value.into())
    }
}

impl From<u64> for CommandErrorArg {
    fn from(value: u64) -> Self {
        Self::Number(value)
    }
}

impl From<bool> for CommandErrorArg {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

#[derive(Clone, Debug, Eq, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandError {
    schema_version: u16,
    code: Box<str>,
    args: BTreeMap<Box<str>, CommandErrorArg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostic_id: Option<Box<str>>,
}

impl CommandError {
    pub(crate) fn new(code: impl Into<Box<str>>) -> Self {
        Self {
            schema_version: COMMAND_ERROR_SCHEMA_VERSION,
            code: code.into(),
            args: BTreeMap::new(),
            diagnostic_id: None,
        }
    }

    pub(crate) fn with_arg(
        mut self,
        name: impl Into<Box<str>>,
        value: impl Into<CommandErrorArg>,
    ) -> Self {
        self.args.insert(name.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_a_stable_semantic_error_contract() {
        let error = CommandError::new("settings.write_failed")
            .with_arg("retryable", true)
            .with_arg("attempt", 2_u64);

        let value = serde_json::to_value(error).expect("serialize command error");

        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["code"], "settings.write_failed");
        assert_eq!(value["args"]["retryable"], true);
        assert_eq!(value["args"]["attempt"], 2);
        assert!(value.get("diagnosticId").is_none());
    }
}
