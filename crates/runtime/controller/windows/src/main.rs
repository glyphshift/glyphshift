use glyphshift_controller_sdk::serve_stdio;
use glyphshift_controller_windows::WindowsController;

fn main() -> std::io::Result<()> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments
        .first()
        .is_some_and(|arg| arg == "--inspect-adapter")
    {
        if arguments.len() != 2 {
            return Err(std::io::Error::other("expected one trusted adapter path"));
        }
        let path = std::path::Path::new(&arguments[1]);
        if glyphshift_adapter_native_host::inspect_pe_architecture(path)? != std::env::consts::ARCH
        {
            return Err(std::io::Error::other(
                "adapter and inspector architectures differ",
            ));
        }
        // Explicit build-tool invocation; the caller supplies its just-built native package.
        let metadata =
            unsafe { glyphshift_adapter_native_host::NativeAdapterMetadata::inspect(path) }
                .map_err(|error| {
                    std::io::Error::other(format!("native inspection failed: {error:?}"))
                })?;
        println!("{}", serde_json::to_string(&metadata)?);
        return Ok(());
    }
    if !arguments.is_empty() {
        return Err(std::io::Error::other("unknown controller mode"));
    }
    serve_stdio(WindowsController::new())
}
