use std::io::{self, BufRead, Write};

fn main() -> io::Result<()> {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let mut output = io::stdout().lock();
    output.write_all(b"{malformed-response}\n")?;
    output.flush()
}
