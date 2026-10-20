fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let code = apt_probe::cli(&args, &mut std::io::stdout(), &mut std::io::stderr());
    std::process::exit(code);
}
