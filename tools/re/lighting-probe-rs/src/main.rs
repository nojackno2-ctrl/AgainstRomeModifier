fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let code = lighting_probe::cli(
        &args,
        std::io::stdin(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    );
    std::process::exit(code);
}
