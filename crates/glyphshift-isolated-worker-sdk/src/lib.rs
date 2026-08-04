//! Versioned stdio protocol and authoring surface for isolated observation workers.

use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

pub const PROTOCOL_SCHEMA: &str = "glyphshift.isolated-worker/1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerActivation {
    pub adapter_id: String,
    pub target_grant: WorkerTargetGrant,
    pub producer_id: String,
    pub producer_generation: u64,
    pub publication_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerTargetGrant {
    pub platform: String,
    pub payload: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEnvelope {
    pub schema: String,
    pub request_id: u64,
    pub request: Request,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Handshake { activation: WorkerActivation },
    UpdateGeneration { publication_generation: u64 },
    ControlCapture { paused: bool },
    QueryObservations,
    QueryHealth,
    Deactivate,
    Terminate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseEnvelope {
    pub schema: String,
    pub request_id: u64,
    pub response: Response,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Hello {
        adapter_id: String,
        producer_id: String,
        producer_generation: u64,
        publication_generation: u64,
    },
    GenerationApplied {
        publication_generation: u64,
    },
    CaptureControlled {
        paused: bool,
    },
    Observations {
        batch_json: String,
    },
    Health {
        report: WireWorkerHealthReport,
    },
    Deactivated {
        producer_generation: u64,
    },
    Error {
        code: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireWorkerHealth {
    Healthy,
    Degraded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireWorkerHealthReport {
    pub state: WireWorkerHealth,
    pub code: Option<String>,
}

impl WireWorkerHealthReport {
    #[must_use]
    pub const fn healthy() -> Self {
        Self {
            state: WireWorkerHealth::Healthy,
            code: None,
        }
    }

    #[must_use]
    pub fn degraded(code: impl Into<String>) -> Self {
        Self {
            state: WireWorkerHealth::Degraded,
            code: Some(code.into()),
        }
    }
}

pub trait IsolatedWorker {
    fn activate(&mut self, activation: &WorkerActivation) -> Result<(), WorkerError>;

    fn control_capture(&mut self, paused: bool) -> Result<(), WorkerError>;

    fn update_generation(&mut self, publication_generation: u64) -> Result<u64, WorkerError>;

    /// Returns a `glyphshift.capture-observation-batch/1` JSON document.
    fn query_observations(&mut self) -> Result<String, WorkerError>;

    fn health(&mut self) -> Result<WireWorkerHealthReport, WorkerError>;

    /// Removes provider callbacks and returns the generation that was deactivated.
    /// Observation queries remain available until the host terminates the process.
    fn deactivate(&mut self) -> Result<u64, WorkerError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerError {
    code: String,
}

impl WorkerError {
    #[must_use]
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

pub fn serve_stdio(worker: impl IsolatedWorker) -> io::Result<()> {
    serve(
        worker,
        io::BufReader::new(io::stdin().lock()),
        io::stdout().lock(),
    )
}

pub fn serve(
    mut worker: impl IsolatedWorker,
    input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        let request = match serde_json::from_str::<RequestEnvelope>(&line) {
            Ok(request) if request.schema == PROTOCOL_SCHEMA => request,
            Ok(_) | Err(_) => continue,
        };
        let request_id = request.request_id;
        let response = match request.request {
            Request::Handshake { activation } => worker
                .activate(&activation)
                .map(|()| Response::Hello {
                    adapter_id: activation.adapter_id,
                    producer_id: activation.producer_id,
                    producer_generation: activation.producer_generation,
                    publication_generation: activation.publication_generation,
                })
                .unwrap_or_else(worker_error),
            Request::UpdateGeneration {
                publication_generation,
            } => worker
                .update_generation(publication_generation)
                .map(|publication_generation| Response::GenerationApplied {
                    publication_generation,
                })
                .unwrap_or_else(worker_error),
            Request::ControlCapture { paused } => worker
                .control_capture(paused)
                .map(|()| Response::CaptureControlled { paused })
                .unwrap_or_else(worker_error),
            Request::QueryObservations => worker
                .query_observations()
                .map(|batch_json| Response::Observations { batch_json })
                .unwrap_or_else(worker_error),
            Request::QueryHealth => worker
                .health()
                .map(|report| Response::Health { report })
                .unwrap_or_else(worker_error),
            Request::Deactivate => worker
                .deactivate()
                .map(|producer_generation| Response::Deactivated {
                    producer_generation,
                })
                .unwrap_or_else(worker_error),
            Request::Terminate => break,
        };
        serde_json::to_writer(
            &mut output,
            &ResponseEnvelope {
                schema: PROTOCOL_SCHEMA.into(),
                request_id,
                response,
            },
        )?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}

fn worker_error(error: WorkerError) -> Response {
    Response::Error { code: error.code }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct SyntheticWorker {
        producer_generation: u64,
        publication_generation: u64,
    }

    impl IsolatedWorker for SyntheticWorker {
        fn activate(&mut self, activation: &WorkerActivation) -> Result<(), WorkerError> {
            self.producer_generation = activation.producer_generation;
            self.publication_generation = activation.publication_generation;
            Ok(())
        }

        fn control_capture(&mut self, _paused: bool) -> Result<(), WorkerError> {
            Ok(())
        }

        fn update_generation(&mut self, publication_generation: u64) -> Result<u64, WorkerError> {
            self.publication_generation = publication_generation;
            Ok(publication_generation)
        }

        fn query_observations(&mut self) -> Result<String, WorkerError> {
            Ok("{\"schema\":\"synthetic\"}".into())
        }

        fn health(&mut self) -> Result<WireWorkerHealthReport, WorkerError> {
            Ok(WireWorkerHealthReport::healthy())
        }

        fn deactivate(&mut self) -> Result<u64, WorkerError> {
            Ok(self.producer_generation)
        }
    }

    #[test]
    fn worker_protocol_round_trips_lifecycle_with_request_identity() {
        let requests = [
            Request::Handshake {
                activation: WorkerActivation {
                    adapter_id: "synthetic.uia".into(),
                    target_grant: WorkerTargetGrant {
                        platform: "synthetic-process-v1".into(),
                        payload: "target:opaque".into(),
                    },
                    producer_id: "worker-1".into(),
                    producer_generation: 7,
                    publication_generation: 11,
                },
            },
            Request::UpdateGeneration {
                publication_generation: 12,
            },
            Request::QueryHealth,
            Request::Deactivate,
        ];
        let mut input = Vec::new();
        for (index, request) in requests.into_iter().enumerate() {
            serde_json::to_writer(
                &mut input,
                &RequestEnvelope {
                    schema: PROTOCOL_SCHEMA.into(),
                    request_id: index as u64 + 1,
                    request,
                },
            )
            .expect("request");
            input.push(b'\n');
        }
        let mut output = Vec::new();
        serve(
            SyntheticWorker {
                producer_generation: 0,
                publication_generation: 0,
            },
            Cursor::new(input),
            &mut output,
        )
        .expect("serve worker");
        let responses = String::from_utf8(output).expect("utf8 responses");
        let decoded = responses
            .lines()
            .map(|line| serde_json::from_str::<ResponseEnvelope>(line).expect("response"))
            .collect::<Vec<_>>();
        assert_eq!(decoded.len(), 4);
        assert_eq!(decoded[0].request_id, 1);
        assert!(matches!(
            decoded[0].response,
            Response::Hello {
                producer_generation: 7,
                publication_generation: 11,
                ..
            }
        ));
        assert!(matches!(
            decoded[1].response,
            Response::GenerationApplied {
                publication_generation: 12
            }
        ));
        assert!(matches!(
            decoded[2].response,
            Response::Health {
                report: WireWorkerHealthReport {
                    state: WireWorkerHealth::Healthy,
                    ..
                }
            }
        ));
        assert_eq!(
            decoded[3].response,
            Response::Deactivated {
                producer_generation: 7
            }
        );
    }
}
