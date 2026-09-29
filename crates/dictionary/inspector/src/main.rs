//! Static Registry admission checks. Input arrives on stdin; DLLs are never loaded.
use glyphshift_resource_inspector::inspect;
use std::io::Read;

fn run() -> Result<(), &'static str> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [kind] = args.as_slice() else {
        return Err("usage: glyphshift-resource-inspector <dictionary|adapter> < artifact");
    };
    let limit = match kind.as_str() {
        "dictionary" => 16 * 1024 * 1024,
        "adapter" => 64 * 1024 * 1024,
        _ => return Err("unsupported_kind"),
    };
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "read_failed")?;
    if bytes.len() as u64 > limit {
        return Err("size_limit");
    }
    println!("{}", inspect(kind, &bytes)?);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_archives_and_unknown_types_are_rejected() {
        assert!(inspect("adapter", b"not a ZIP").is_err());
        assert!(inspect("dictionary", b"{}").is_err());
        assert!(inspect("unknown", b"{}").is_err());
    }
}
