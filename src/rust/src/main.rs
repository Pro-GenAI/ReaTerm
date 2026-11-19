fn main() {
    if let Err(error) = reaterm::run(std::env::args().skip(1).collect()) {
        eprintln!("reaterm: {error}");
        std::process::exit(2);
    }
}
