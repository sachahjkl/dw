use clap::Parser;

mod ado;
mod cli;
mod completion;
mod db;
mod doctor;
mod handlers;
mod simple_handlers;
mod upgrade;
mod version;

fn main() {
    if let Err(error) = handlers::run(cli::Cli::parse()) {
        eprintln!("Erreur: {error}");
        std::process::exit(1);
    }
}
