mod app;
mod auto_restart;
mod config;
mod event_handler;
mod podman;
mod service;
mod status;
mod systemd;
mod toast;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use ratatui::crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::CrosstermBackend;
use ratatui::{DefaultTerminal, TerminalOptions, Viewport};

use app::App;
use config::Keybinds;

struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}

fn install_panic_hook() {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
        previous_hook(panic_info);
    }));
}

fn main() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(async {
        // Terminal input, rendering and legacy CLI calls are blocking. Keep the
        // UI on the blocking pool so Tokio's async workers remain available.
        tokio::task::spawn_blocking(|| tokio::runtime::Handle::current().block_on(run_app()))
            .await?
    });
    // A stuck external command must not prevent the terminal app from exiting.
    runtime.shutdown_timeout(Duration::from_secs(2));
    result
}

async fn run_app() -> Result<()> {
    match command_mode()? {
        CommandMode::AutoRestart(project_root) => {
            auto_restart::start_configured_services(&project_root)?;
            return Ok(());
        }
        CommandMode::InstallAutoRestart(project_root) => {
            let unit_name = systemd::install_auto_restart_unit(&project_root)?;
            println!("Installed and enabled {unit_name}");
            return Ok(());
        }
        CommandMode::RemoveAutoRestart => {
            systemd::remove_auto_restart_unit()?;
            println!("Removed auto-restart unit");
            return Ok(());
        }
        CommandMode::Tui => {}
    }

    install_panic_hook();

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let _terminal_cleanup = TerminalCleanup;

    let terminal = ratatui::Terminal::with_options(
        CrosstermBackend::new(stdout),
        TerminalOptions {
            viewport: Viewport::Fullscreen,
        },
    )?;

    run(terminal).await?;
    Ok(())
}

enum CommandMode {
    Tui,
    AutoRestart(PathBuf),
    InstallAutoRestart(PathBuf),
    RemoveAutoRestart,
}

fn command_mode() -> Result<CommandMode> {
    let mut args = std::env::args_os().skip(1);
    let Some(command) = args.next() else {
        return Ok(CommandMode::Tui);
    };

    match command.to_str() {
        Some("--auto-restart") => {
            let project_root = args
                .next()
                .map(PathBuf::from)
                .context("--auto-restart requires a project root path")?;
            if args.next().is_some() {
                bail!("--auto-restart accepts exactly one project root path");
            }
            Ok(CommandMode::AutoRestart(project_root))
        }
        Some("--install-auto-restart") => {
            let project_root = args
                .next()
                .map(PathBuf::from)
                .map(Ok)
                .unwrap_or_else(std::env::current_dir)?;
            if args.next().is_some() {
                bail!("--install-auto-restart accepts at most one project root path");
            }
            Ok(CommandMode::InstallAutoRestart(project_root))
        }
        Some("--remove-auto-restart") => {
            if args.next().is_some() {
                bail!("--remove-auto-restart takes no arguments");
            }
            Ok(CommandMode::RemoveAutoRestart)
        }
        Some(unknown) => bail!("unknown argument: {unknown}"),
        None => bail!("command argument is not valid UTF-8"),
    }
}

async fn run(mut terminal: DefaultTerminal) -> io::Result<()> {
    let keybinds = Keybinds::load();
    let mut app = App::new(keybinds);
    app.next();

    let result = run_loop(&mut terminal, &mut app).await;
    app.stop_event_listeners();
    app.kill_all_live_logs();
    result
}

async fn run_loop(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    const FRAME_DURATION: Duration = Duration::from_millis(33);
    loop {
        let mut render_error: Option<io::Error> = None;
        terminal.draw(|frame| {
            if let Err(err) = ui::render_ui(frame, app) {
                render_error = Some(err);
            }
        })?;

        if let Some(err) = render_error {
            return Err(err);
        }

        if !event_handler::handle_events(app, FRAME_DURATION).await? {
            break;
        }
    }

    Ok(())
}
