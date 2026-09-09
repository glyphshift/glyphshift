use super::*;
// Keep subtitle text and line breaks; cue numbers and timing are structural only.
pub(super) fn decode(text: &str) -> Result<Vec<(usize, Entry)>, TransferError> {
    fn timestamp(value: &str) -> Option<u64> {
        let parts = value.split([':', ',']).collect::<Vec<_>>();
        if parts.len() != 4
            || parts[0].len() < 2
            || parts[1].len() != 2
            || parts[2].len() != 2
            || parts[3].len() != 3
            || parts
                .iter()
                .any(|part| !part.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return None;
        }
        let hours = parts[0].parse::<u64>().ok()?;
        let minutes = parts[1].parse::<u64>().ok()?;
        let seconds = parts[2].parse::<u64>().ok()?;
        let millis = parts[3].parse::<u64>().ok()?;
        if minutes >= 60 || seconds >= 60 {
            return None;
        }
        hours
            .checked_mul(3_600_000)?
            .checked_add(minutes * 60_000 + seconds * 1_000 + millis)
    }
    let lines = text.lines().enumerate().collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut cursor = 0;
    while cursor < lines.len() {
        if lines[cursor].1.trim().is_empty() {
            cursor += 1;
            continue;
        }
        let (number, index) = lines[cursor];
        if !index.trim().bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(at_line("import.srt_index", number + 1));
        }
        cursor += 1;
        let (line, timing) = lines
            .get(cursor)
            .copied()
            .ok_or_else(|| at_line("import.srt_timing", number + 2))?;
        let valid = timing
            .split_once("-->")
            .and_then(|(start, end)| {
                let start = timestamp(start.trim())?;
                let end = timestamp(end.split_whitespace().next()?)?;
                Some(end >= start)
            })
            .unwrap_or(false);
        if !valid {
            return Err(at_line("import.srt_timing", line + 1));
        }
        cursor += 1;
        let start = cursor;
        while cursor < lines.len() && !lines[cursor].1.trim().is_empty() {
            if lines[cursor].1.contains("-->") {
                return Err(at_line("import.srt_separator", lines[cursor].0 + 1));
            }
            cursor += 1;
        }
        if start == cursor {
            return Err(at_line("import.srt_text", line + 2));
        }
        let source = lines[start..cursor]
            .iter()
            .map(|(_, text)| text.trim())
            .collect::<Vec<_>>()
            .join("\n");
        result.push((
            lines[start].0 + 1,
            Entry {
                source,
                translation: String::new(),
            },
        ));
    }
    Ok(result)
}
