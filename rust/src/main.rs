//! CodexBar - Windows system tray app for monitoring AI provider usage limits
//!
//! This is a Rust port of the macOS CodexBar application, providing:
//! - System tray icon with usage status (via `codexbar menubar`)
//! - CLI for querying usage from terminal (default command)
//! - Support for multiple AI providers (Claude, Codex, Gemini, etc.)

mod browser;
mod cli;
mod core;
mod cost_scanner;
mod host;
mod logging;
mod login;
mod native_ui;
mod notifications;
mod providers;
mod settings;
mod shortcuts;
mod single_instance;
mod sound;
mod status;
mod tray;
mod updater;

use clap::Parser;
use cli::{exit_codes, Cli, Commands};

fn main() {
    let exit_code = run();
    std::process::exit(exit_code);
}

fn run() -> i32 {
    let cli = Cli::parse();

    // Initialize logging
    if let Err(e) = logging::init(cli.verbose, cli.json_output) {
        eprintln!("Failed to initialize logging: {}", e);
        return exit_codes::UNEXPECTED_FAILURE;
    }

    // Create tokio runtime for async commands
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Failed to create runtime: {}", e);
            return exit_codes::UNEXPECTED_FAILURE;
        }
    };

    match cli.command {
        Some(Commands::Usage(args)) => {
            rt.block_on(async {
                match cli::usage::run(args).await {
                    Ok(()) => exit_codes::SUCCESS,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        categorize_error(&e)
                    }
                }
            })
        }
        Some(Commands::Cost(args)) => {
            rt.block_on(async {
                match cli::cost::run(args).await {
                    Ok(()) => exit_codes::SUCCESS,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        categorize_error(&e)
                    }
                }
            })
        }
        Some(Commands::Menubar) => {
            // Hide the console window for GUI mode
            #[cfg(windows)]
            hide_console_window();

            // Check for existing instance
            let _guard = match single_instance::SingleInstanceGuard::try_acquire() {
                Some(guard) => guard,
                None => {
                    // Can't print to console anymore, just exit
                    return exit_codes::SUCCESS;
                }
            };

            match native_ui::run() {
                Ok(()) => exit_codes::SUCCESS,
                Err(_) => exit_codes::UNEXPECTED_FAILURE,
            }
        }
        Some(Commands::Autostart(args)) => {
            rt.block_on(async {
                match cli::autostart::run(args).await {
                    Ok(()) => exit_codes::SUCCESS,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        exit_codes::UNEXPECTED_FAILURE
                    }
                }
            })
        }
        Some(Commands::Account(args)) => {
            rt.block_on(async {
                match cli::account::run(args).await {
                    Ok(()) => exit_codes::SUCCESS,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        exit_codes::UNEXPECTED_FAILURE
                    }
                }
            })
        }
        Some(Commands::Config(args)) => {
            rt.block_on(async {
                match cli::config::run(args).await {
                    Ok(()) => exit_codes::SUCCESS,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        exit_codes::UNEXPECTED_FAILURE
                    }
                }
            })
        }
        None => {
            if cli.has_usage_flags() {
                // CLI flags present → run usage command
                let args = cli.to_usage_args();
                rt.block_on(async {
                    match cli::usage::run(args).await {
                        Ok(()) => exit_codes::SUCCESS,
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            categorize_error(&e)
                        }
                    }
                })
            } else {
                // No flags (bare `codexbar`, e.g. double-click) → launch GUI
                #[cfg(windows)]
                hide_console_window();

                let _guard = match single_instance::SingleInstanceGuard::try_acquire() {
                    Some(guard) => guard,
                    None => {
                        return exit_codes::SUCCESS;
                    }
                };

                match native_ui::run() {
                    Ok(()) => exit_codes::SUCCESS,
                    Err(_) => exit_codes::UNEXPECTED_FAILURE,
                }
            }
        }
    }
}

/// Categorize an error into the appropriate exit code
fn categorize_error(e: &anyhow::Error) -> i32 {
    let msg = e.to_string().to_lowercase();

    if msg.contains("not installed") || msg.contains("not found") || msg.contains("binary") {
        exit_codes::PROVIDER_MISSING
    } else if msg.contains("parse") || msg.contains("format") || msg.contains("invalid") {
        exit_codes::PARSE_ERROR
    } else if msg.contains("timeout") || msg.contains("timed out") {
        exit_codes::CLI_TIMEOUT
    } else {
        exit_codes::UNEXPECTED_FAILURE
    }
}

/// Hide the console window on Windows (for GUI mode)
#[cfg(windows)]
fn hide_console_window() {
    use windows::Win32::System::Console::GetConsoleWindow;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

    unsafe {
        let console = GetConsoleWindow();
        if !console.is_invalid() {
            let _ = ShowWindow(console, SW_HIDE);
        }
    }
}
