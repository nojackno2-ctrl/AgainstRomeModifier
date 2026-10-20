fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    std::process::exit(alr_scanline_differential::cli(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ));
}
