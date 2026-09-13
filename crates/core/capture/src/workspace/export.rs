use super::*;

impl ProbeRunStore {
    pub fn export(
        &mut self,
        run_id: &str,
        format: ProbeExportFormat,
        dictionary: &ProbeDictionarySnapshot,
    ) -> Result<Vec<u8>, ProbeRunError> {
        let document = self.synchronized_document(run_id)?;
        match format {
            ProbeExportFormat::EntriesJson => {
                let entries = self.combined_rows(&document, dictionary)?.into_iter().map(|row|
                    serde_json::json!({"source": row.source, "translation": row.translation})).collect::<Vec<_>>();
                serde_json::to_vec_pretty(&serde_json::json!({"schema": "glyphshift.probe-entries/1", "entries": entries})).map_err(|_| ProbeRunError::Export)
            }
            ProbeExportFormat::ObservationsJson => self
                .read_observations(run_id)?
                .encode_json()
                .map(String::into_bytes)
                .map_err(|_| ProbeRunError::Export),
            ProbeExportFormat::ObservationsCsv => {
                let observations = self.read_observations(run_id)?;
                let mut output =
                    String::from("\u{feff}source,adapterId,count,firstSeenMs,lastSeenMs\r\n");
                for entry in observations.entries() {
                    push_csv_row(
                        &mut output,
                        [
                            entry.source().to_owned(),
                            entry.adapter_id().to_owned(),
                            entry.count().to_string(),
                            entry.first_seen_ms().to_string(),
                            entry.last_seen_ms().to_string(),
                        ],
                    );
                }
                Ok(output.into_bytes())
            }
            ProbeExportFormat::EntriesCsv => {
                let mut output = String::from(
                    "\u{feff}source,translation,state,adapterIds,count,firstSeenMs,lastSeenMs\r\n",
                );
                for row in self.combined_rows(&document, dictionary)? {
                    push_csv_row(
                        &mut output,
                        [
                            row.source.to_string(),
                            row.translation.to_string(),
                            state_name(row.state).to_owned(),
                            row.adapter_ids.join(" | "),
                            row.count.to_string(),
                            row.first_seen_ms.to_string(),
                            row.last_seen_ms.to_string(),
                        ],
                    );
                }
                Ok(output.into_bytes())
            }
            ProbeExportFormat::DictionaryJson => Err(ProbeRunError::InvalidInput),
        }
    }
}

fn state_name(state: ProbeEntryState) -> &'static str {
    match state {
        ProbeEntryState::Pending => "pending",
        ProbeEntryState::Translated => "translated",
        ProbeEntryState::Unobserved => "unobserved",
        ProbeEntryState::Ignored => "ignored",
    }
}

fn push_csv_row<const N: usize>(output: &mut String, fields: [String; N]) {
    for (index, field) in fields.into_iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push('"');
        output.push_str(&field.replace('"', "\"\""));
        output.push('"');
    }
    output.push_str("\r\n");
}
