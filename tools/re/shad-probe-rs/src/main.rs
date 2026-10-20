fn main() {
    std::process::exit(shad_probe::cli(std::env::args_os().skip(1).collect()));
}
