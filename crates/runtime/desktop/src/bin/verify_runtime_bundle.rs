use glyphshift_desktop_runtime::RuntimeBundle;

fn main() {
    let Some(root) = std::env::args_os().nth(1) else {
        eprintln!("usage: glyphshift-runtime-bundle-verify <runtime-bundle-root>");
        std::process::exit(2);
    };
    match RuntimeBundle::open(root) {
        Ok(bundle) if !bundle.adapter_options().is_empty() => {
            println!(
                "Runtime Bundle verified: {} adapters",
                bundle.adapter_options().len()
            );
        }
        Ok(_) => {
            eprintln!("Runtime Bundle verification failed: empty adapter catalog");
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("Runtime Bundle verification failed: {error:?}");
            std::process::exit(1);
        }
    }
}
