fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match ar_re_tools::cmd::dispatch(&args) {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(if message.starts_with("usage:") { 2 } else { 1 });
        }
    }
}
