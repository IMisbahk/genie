use clap::Parser;
use genie::{Cli, Commands, LogOptions, StatusOptions};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Ui { port }) => {
            genie::startUiServer(*port).await;
        }
        Some(Commands::Init) => {
            if let Err(e) = genie::initProject() {
                eprintln!("Initialization failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Status {
            jsonMode,
            includeFiles,
            deepCompare,
        }) => {
            genie::showStatus(StatusOptions {
                jsonMode: *jsonMode,
                includeFiles: *includeFiles,
                deepCompare: *deepCompare,
            });
        }
        Some(Commands::Commit { message }) => match message {
            Some(msg) => {
                if let Err(e) = genie::makeCommit(msg) {
                    eprintln!("Commit failed: {}", e);
                }
            }
            None => {
                println!("Example: genie commit -m \"Initial commit\"");
            }
        },
        Some(Commands::Log { jsonMode, limit }) => {
            genie::showLog(LogOptions {
                jsonMode: *jsonMode,
                limit: *limit,
            });
        }
        Some(Commands::Watch {
            intervalSeconds,
            includeFiles,
            deepCompare,
        }) => {
            genie::watchStatus(*intervalSeconds, *includeFiles, *deepCompare).await;
        }
        Some(Commands::Guard {
            maxFileMegabytes,
            jsonMode,
            strictMode,
        }) => {
            genie::runGuard(*maxFileMegabytes, *jsonMode, *strictMode);
        }
        Some(Commands::Insights { jsonMode, topFiles }) => {
            genie::showInsights(*jsonMode, *topFiles);
        }
        Some(Commands::Projects {
            jsonMode,
            includeDetails,
        }) => {
            genie::showProjects(*jsonMode, *includeDetails);
        }
        Some(Commands::Completions { shell }) => {
            genie::generateCompletions(shell);
        }
        Some(Commands::Man) => {
            genie::printMan();
        }
        Some(Commands::SelfUpdate) => {
            genie::doSelfUpdate();
        }
        Some(Commands::Welcome) => {
            genie::showWelcome();
        }
        Some(Commands::Docs) => {
            genie::openDocs();
        }
        None => {
            println!("🧞‍♂️ Welcome to Genie!");
            println!("Your personal version control system.");
            println!("Run `genie --help` to explore the new guard/watch/insights commands.");
        }
    }
}
