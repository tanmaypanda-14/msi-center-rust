//! MSI Center replacement — application shell.
//!
//! One process: a Slint window reading the EC through `msi-core`, refreshing
//! once a second, themed from the active Omarchy theme. No webview, no runtime,
//! no background services.

mod cli;
mod headless;
mod history;
mod theme;
mod ui;
mod update;
mod view;
mod wiring;

use std::sync::Arc;

use msi_core::{default_backend, MsiControl};
use msi_theme::Theme as SystemTheme;
use slint::ComponentHandle;

use cli::Command;
use ui::{AppState, AppWindow};

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let command = match cli::parse(&args) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    if let Command::Gui { page } = command {
        return run(page);
    }

    let backend: Arc<dyn MsiControl> = Arc::from(default_backend());
    let code = match command {
        Command::Help => {
            println!("{}", cli::USAGE);
            0
        }
        Command::Version => {
            println!("msi-center {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Command::Snapshot => headless::snapshot(backend.as_ref()),
        Command::Set { key, value } => headless::set(backend.as_ref(), &key, &value),
        Command::Theme => headless::theme(&SystemTheme::load()),
        Command::Update {
            dry_run,
            assume_yes,
        } => update::run(update::Options {
            dry_run,
            assume_yes,
        }),
        Command::Gui { .. } => unreachable!("handled above"),
    };

    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

/// Open the window. Returns when the user closes it.
fn run(page: i32) -> Result<(), slint::PlatformError> {
    let backend: Arc<dyn MsiControl> = Arc::from(default_backend());
    let system_theme = SystemTheme::load();

    let ui = AppWindow::new()?;

    // Give the window a real Wayland app_id so the compositor can apply rules
    // and group it like any other app (must happen before the window is shown).
    let _ = slint::set_xdg_app_id("msi-center");

    theme::apply(&ui, &system_theme);

    {
        let state = ui.global::<AppState>();
        state.set_theme_name(system_theme.name.as_str().into());
        state.set_backend_label(backend.name().into());
        state.set_page(page);
    }

    view::refresh(&ui, backend.as_ref());

    // The timer stops when it is dropped, so it has to outlive `run()`.
    let _refresh = wiring::connect(&ui, backend);

    ui.run()
}
