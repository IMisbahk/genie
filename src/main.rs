use clap::Parser;
use genie::{Cli, Commands};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Ui { port }) => {
            genie::start_ui_server(*port).await;
        }
        Some(Commands::Init) => {
            if let Err(e) = genie::init_project() {
                eprintln!("Initialization failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Status) => {
            genie::show_status();
        }
        Some(Commands::Commit { message }) => {
            match message {
                Some(msg) => {
                    if let Err(e) = genie::make_commit(msg) {
                        eprintln!("Commit failed: {}", e);
                    }
                }
                None => {
                    println!("Example: genie commit -m \"Initial commit\"");
                }
            }
        }
        Some(Commands::Log) => {
            genie::show_log();
        }
        Some(Commands::Completions { shell }) => {
            genie::generate_completions(shell);
        }
        Some(Commands::Man) => {
            genie::print_man();
        }
        Some(Commands::SelfUpdate) => {
            genie::do_self_update();
        }
        Some(Commands::Welcome) => {
            genie::show_welcome();
        }
        Some(Commands::Docs) => {
            genie::open_docs();
        }
        None => {
            println!("🧞‍♂️ Welcome to Genie!");
            println!("Your personal version control system.");
            println!("Run `genie --help` to see all available commands and options.");
            println!("Usage: genie <command> [options]");
            println!();
            println!("Available commands:");
            println!("  init       Initialize a new Genie project");
            println!("  status     Show current project status");
            println!("  commit     Commit changes (-m \"message\")");
            println!("  log        Show commit history");
            println!("  ui         Launch the Genie UI dashboard");
            println!("  completions <shell>  Print shell completions (bash|zsh|fish)");
            println!("  man        Print the CLI manual page");
            println!("  self-update  Update to latest release");
            println!();
        }
    }
}