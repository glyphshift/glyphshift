fn main() -> std::io::Result<()> {
    glyphshift_acquisition_worker_sdk::serve_stdio(
        glyphshift_adapter_uia_worker::WindowsUiaAcquisitionWorker,
    )
}
