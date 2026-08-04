fn main() -> std::io::Result<()> {
    glyphshift_isolated_worker_sdk::serve_stdio(
        glyphshift_adapter_uia_worker::WindowsUiaWorker::default(),
    )
}
