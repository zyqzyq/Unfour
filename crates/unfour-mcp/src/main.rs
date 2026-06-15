use std::io;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    if let Err(error) = unfour_mcp::run_stdio(stdin.lock(), stdout.lock()) {
        eprintln!("unfour-mcp stdio server failed: {error}");
        std::process::exit(1);
    }
}
