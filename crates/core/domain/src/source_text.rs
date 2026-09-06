use std::borrow::Cow;

/// Declared by the text producer, not guessed by the dictionary or by app name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceTextPolicy {
    #[default]
    Exact,
    /// A single ASCII space immediately before a single line break between
    /// Latin words is layout padding. Other breaks and whitespace stay intact.
    SpacePaddedSoftWrap,
}

impl SourceTextPolicy {
    pub fn from_code(code: u32) -> Option<Self> {
        match code { 0 => Some(Self::Exact), 1 => Some(Self::SpacePaddedSoftWrap), _ => None }
    }

    pub fn normalize<'a>(self, source: &'a str) -> Cow<'a, str> {
        if self == Self::Exact { return Cow::Borrowed(source); }
        let bytes = source.as_bytes();
        let mut output = String::new();
        let mut copied = 0;
        let mut i = 0;
        while i < bytes.len() {
            let end = if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') { i + 2 }
                else if bytes[i] == b'\n' { i + 1 } else { i += 1; continue };
            let previous_word = i >= 2 && (bytes[i - 2].is_ascii_alphanumeric() || b",.;!?)]'\"".contains(&bytes[i - 2]));
            if previous_word && bytes[i - 1] == b' ' && bytes.get(end).is_some_and(u8::is_ascii_alphanumeric) {
                output.push_str(&source[copied..i]);
                copied = end;
            }
            i = end;
        }
        if copied == 0 { Cow::Borrowed(source) }
        else { output.push_str(&source[copied..]); Cow::Owned(output) }
    }

    pub fn key(self, source: &str) -> String { self.normalize(source).trim().to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrap_positions_share_a_key_without_removing_paragraphs() {
        let p = SourceTextPolicy::SpacePaddedSoftWrap;
        assert_eq!(p.key("Strong flexible \r\nmaterial for tools."), "Strong flexible material for tools.");
        assert_eq!(p.key("Strong \nflexible material \r\nfor tools."), "Strong flexible material for tools.");
        for hard in ["Title\nBody", "Title \n\nBody", "Title \n Body", "Title  \nBody", "Title:\nBody", "Title: \nBody", "A \n- item"] {
            assert_eq!(p.normalize(hard), hard);
        }
        let normalized = p.normalize("One \r\ntwo \nthree");
        assert_eq!(p.normalize(&normalized), normalized);
        assert_eq!(SourceTextPolicy::Exact.normalize("One \r\ntwo"), "One \r\ntwo");
    }
}
