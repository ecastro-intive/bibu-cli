use clap::Parser;

fn main() {
    let cli = bibu::cli::Cli::parse();
    std::process::exit(bibu::run(&cli));
}
