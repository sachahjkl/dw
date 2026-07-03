mod cli;
mod completion;
mod doctor;
mod handlers;
mod upgrade;
mod version;

fn main() {
    if let Err(error) = handlers::run(cli::Cli::parse_localized()) {
        eprintln!("Erreur: {error}");
        std::process::exit(1);
    }
}
