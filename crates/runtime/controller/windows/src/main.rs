use glyphshift_controller_sdk::serve_stdio;
use glyphshift_controller_windows::WindowsController;

fn main() -> std::io::Result<()> {
    serve_stdio(WindowsController::new())
}
