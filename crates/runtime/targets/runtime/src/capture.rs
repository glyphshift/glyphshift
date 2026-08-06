use crate::TargetRuntimeError;
use glyphshift_capture::{
    CaptureBatchIngress, CaptureBatchProducer, CaptureConfiguration, CaptureIngress,
    CaptureObservationBatch, CaptureProducerConfiguration, FileCaptureSink,
};
use glyphshift_target_runtime_contract::TargetRuntimeDeployment;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RuntimeCaptureConfiguration {
    File(CaptureConfiguration),
    Batch(CaptureProducerConfiguration),
}

impl RuntimeCaptureConfiguration {
    pub(super) fn from_deployment(deployment: &TargetRuntimeDeployment) -> Option<Self> {
        deployment
            .capture()
            .cloned()
            .map(Self::File)
            .or_else(|| deployment.observation_producer().cloned().map(Self::Batch))
    }
}

pub(super) enum RuntimeCapture {
    File {
        configuration: CaptureConfiguration,
        ingress: CaptureIngress,
        owner: FileCaptureSink,
    },
    Batch {
        configuration: CaptureProducerConfiguration,
        ingress: CaptureBatchIngress,
        producer: CaptureBatchProducer,
    },
}

impl RuntimeCapture {
    pub(super) fn start(
        configuration: RuntimeCaptureConfiguration,
    ) -> Result<Self, TargetRuntimeError> {
        match configuration {
            RuntimeCaptureConfiguration::File(configuration) => {
                let owner = FileCaptureSink::start(configuration.clone())
                    .map_err(|_| TargetRuntimeError::Capture)?;
                let ingress = owner.ingress();
                Ok(Self::File {
                    configuration,
                    ingress,
                    owner,
                })
            }
            RuntimeCaptureConfiguration::Batch(configuration) => {
                let (producer, ingress) = CaptureBatchProducer::start(configuration.clone())
                    .map_err(|_| TargetRuntimeError::Capture)?;
                Ok(Self::Batch {
                    configuration,
                    ingress,
                    producer,
                })
            }
        }
    }

    pub(super) fn configuration(&self) -> RuntimeCaptureConfiguration {
        match self {
            Self::File { configuration, .. } => {
                RuntimeCaptureConfiguration::File(configuration.clone())
            }
            Self::Batch { configuration, .. } => {
                RuntimeCaptureConfiguration::Batch(configuration.clone())
            }
        }
    }

    pub(super) fn try_observe(&self, adapter_id: Box<str>, source: Box<str>) {
        match self {
            Self::File { ingress, .. } => {
                let _ = ingress.try_observe(adapter_id, source);
            }
            Self::Batch { ingress, .. } => {
                let _ = ingress.try_observe(adapter_id, source);
            }
        }
    }

    pub(super) fn set_paused(&self, paused: bool) {
        match self {
            Self::File { owner, .. } => owner.set_paused(paused),
            Self::Batch { producer, .. } => producer.set_paused(paused),
        }
    }

    pub(super) fn drain(&mut self) -> Result<CaptureObservationBatch, TargetRuntimeError> {
        match self {
            Self::Batch { producer, .. } => {
                producer.drain().map_err(|_| TargetRuntimeError::Capture)
            }
            Self::File { .. } => Err(TargetRuntimeError::Capture),
        }
    }

    pub(super) fn finish(self) -> Result<(), TargetRuntimeError> {
        match self {
            Self::File { owner, .. } => owner
                .finish()
                .map(|_| ())
                .map_err(|_| TargetRuntimeError::Capture),
            Self::Batch { .. } => Ok(()),
        }
    }
}
