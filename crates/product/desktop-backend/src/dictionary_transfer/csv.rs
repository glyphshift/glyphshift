use super::*;
pub(super) fn decode(text: &str) -> Result<Vec<(usize, Entry)>, TransferError> {
    let rows = parse_csv(text)?;
    let header = rows
        .first()
        .ok_or_else(|| TransferError::new("import.empty"))?;
    let source = header
        .fields
        .iter()
        .position(|key| key == "source")
        .ok_or_else(|| at_line("import.csv_header", header.line))?;
    let translation = header
        .fields
        .iter()
        .position(|key| key == "translation")
        .ok_or_else(|| at_line("import.csv_header", header.line))?;
    if header.fields.iter().collect::<BTreeSet<_>>().len() != header.fields.len() {
        return Err(at_line("import.csv_duplicate_header", header.line));
    }
    rows.iter()
        .skip(1)
        .map(|row| {
            if row.fields.len() != header.fields.len() {
                return Err(at_line("import.csv_columns", row.line)
                    .with_arg("expected", header.fields.len() as u64)
                    .with_arg("actual", row.fields.len() as u64));
            }
            Ok((
                row.line,
                Entry {
                    source: row.fields[source].clone(),
                    translation: row.fields[translation].clone(),
                },
            ))
        })
        .collect::<Result<_, _>>()
}
struct CsvRow {
    line: usize,
    fields: Vec<String>,
}

// Track physical lines, including CRLF and newlines inside quoted fields.
fn parse_csv(text: &str) -> Result<Vec<CsvRow>, TransferError> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut state = 0; // start, unquoted, quoted, closed quote
    let mut line = 1;
    let mut row_line = 1;
    let mut quote_line = 1;
    let mut previous_cr = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        let current_line = line;
        if ch == '\r' || (ch == '\n' && !previous_cr) {
            line += 1;
        }
        previous_cr = ch == '\r';
        if state == 2 {
            if ch == '"' {
                state = 3;
            } else {
                field.push(ch);
            }
            continue;
        }
        if state == 3 && ch == '"' {
            field.push(ch);
            state = 2;
            continue;
        }
        match ch {
            '"' if state == 0 => {
                state = 2;
                quote_line = current_line;
            }
            ',' => {
                row.push(std::mem::take(&mut field));
                state = 0;
            }
            '\r' | '\n' => {
                if ch == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                    previous_cr = false;
                }
                row.push(std::mem::take(&mut field));
                if row.len() != 1 || !row[0].is_empty() {
                    rows.push(CsvRow {
                        line: row_line,
                        fields: std::mem::take(&mut row),
                    });
                } else {
                    row.clear();
                }
                state = 0;
                row_line = line;
            }
            '"' => return Err(at_line("import.csv_quote", current_line)),
            _ if state == 3 => return Err(at_line("import.csv_after_quote", current_line)),
            _ => {
                field.push(ch);
                state = 1;
            }
        }
    }
    if state == 2 {
        return Err(at_line("import.csv_unclosed_quote", quote_line));
    }
    if !field.is_empty() || !row.is_empty() || state == 3 {
        row.push(field);
        rows.push(CsvRow {
            line: row_line,
            fields: row,
        });
    }
    Ok(rows)
}

pub(super) fn encode(
    entries: &[Entry],
    _: Option<&serde_json::Value>,
) -> Result<String, TransferError> {
    let mut output = String::from("\u{feff}source,translation\r\n");
    for entry in entries {
        output.push_str(&format!(
            "\"{}\",\"{}\"\r\n",
            entry.source.replace('"', "\"\""),
            entry.translation.replace('"', "\"\"")
        ));
    }
    Ok(output)
}
