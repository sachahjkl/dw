use clap::Parser;

mod cli;
mod completion;
mod doctor;
mod handlers;
mod simple_handlers;
mod task;
mod upgrade;
mod version;

fn main() {
    if let Err(error) = handlers::run(cli::Cli::parse()) {
        eprintln!("Erreur: {error}");
        std::process::exit(1);
    }
}
