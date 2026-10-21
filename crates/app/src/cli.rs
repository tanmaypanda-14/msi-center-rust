//! Command line: the GUI is the default, headless subcommands are opt-in.

/// What the process was asked to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Run the window, optionally on a given page.
    Gui {
        page: i32,
    },
    Help,
    Version,
    /// Print what the app reads from the EC and exit.
    Snapshot,
    /// Print the theme the app resolved and exit.
    Theme,
    /// Apply one EC setting and exit.
    Set {
        key: String,
        value: String,
    },
    /// Check the latest release and install it.
    Update {
        dry_run: bool,
        assume_yes: bool,
    },
}

pub const USAGE: &str = "\
usage: msi-center [--page N]
       msi-center --snapshot
       msi-center --theme
       msi-center --set key=value
       msi-center --update [--dry-run] [--yes]
       msi-center --version";

/// Parse arguments (without the program name).
///
/// Later flags win in the order checked here: the headless commands are
/// mutually exclusive in practice, and the GUI is the fallback.
pub fn parse(args: &[String]) -> Result<Command, String> {
    let has = |flag: &str| args.iter().any(|arg| arg == flag);

    if has("--version") || has("-V") {
        return Ok(Command::Version);
    }
    if has("--help") || has("-h") {
        return Ok(Command::Help);
    }
    if has("--snapshot") {
        return Ok(Command::Snapshot);
    }
    if has("--theme") {
        return Ok(Command::Theme);
    }
    if has("--update") {
        return Ok(Command::Update {
            dry_run: has("--dry-run"),
            assume_yes: has("--yes") || has("-y"),
        });
    }
    if has("--dry-run") || has("--yes") || has("-y") {
        return Err("--dry-run and --yes are only valid with --update".to_owned());
    }
    if let Some(pos) = args.iter().position(|arg| arg == "--set") {
        let spec = args
            .get(pos + 1)
            .ok_or_else(|| "expected key=value after --set".to_owned())?;
        let (key, value) = spec
            .split_once('=')
            .ok_or_else(|| format!("expected key=value, got {spec:?}"))?;
        if key.is_empty() || value.is_empty() {
            return Err(format!("expected key=value, got {spec:?}"));
        }
        return Ok(Command::Set {
            key: key.to_owned(),
            value: value.to_owned(),
        });
    }

    let page = args
        .iter()
        .position(|arg| arg == "--page")
        .and_then(|pos| args.get(pos + 1))
        .and_then(|page| page.parse::<i32>().ok())
        .unwrap_or(0)
        .clamp(0, 3);

    Ok(Command::Gui { page })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(args: &[&str]) -> Result<Command, String> {
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        parse(&args)
    }

    #[test]
    fn no_arguments_opens_the_first_page() {
        assert_eq!(parse_str(&[]).unwrap(), Command::Gui { page: 0 });
    }

    #[test]
    fn page_is_clamped_to_the_pages_that_exist() {
        assert_eq!(
            parse_str(&["--page", "2"]).unwrap(),
            Command::Gui { page: 2 }
        );
        assert_eq!(
            parse_str(&["--page", "9"]).unwrap(),
            Command::Gui { page: 3 }
        );
        assert_eq!(parse_str(&["--page"]).unwrap(), Command::Gui { page: 0 });
        assert_eq!(
            parse_str(&["--page", "x"]).unwrap(),
            Command::Gui { page: 0 }
        );
    }

    #[test]
    fn version_wins_over_everything() {
        assert_eq!(parse_str(&["--version"]).unwrap(), Command::Version);
        assert_eq!(parse_str(&["-V"]).unwrap(), Command::Version);
        assert_eq!(
            parse_str(&["--snapshot", "--version"]).unwrap(),
            Command::Version
        );
    }

    #[test]
    fn headless_commands_are_recognised() {
        assert_eq!(parse_str(&["--snapshot"]).unwrap(), Command::Snapshot);
        assert_eq!(parse_str(&["--theme"]).unwrap(), Command::Theme);
        assert_eq!(parse_str(&["--help"]).unwrap(), Command::Help);
    }

    #[test]
    fn set_needs_a_key_and_a_value() {
        assert_eq!(
            parse_str(&["--set", "fan=silent"]).unwrap(),
            Command::Set {
                key: "fan".to_owned(),
                value: "silent".to_owned()
            }
        );
        assert!(parse_str(&["--set"]).is_err());
        assert!(parse_str(&["--set", "fan"]).is_err());
        assert!(parse_str(&["--set", "=silent"]).is_err());
    }

    #[test]
    fn update_takes_its_own_flags() {
        assert_eq!(
            parse_str(&["--update"]).unwrap(),
            Command::Update {
                dry_run: false,
                assume_yes: false
            }
        );
        assert_eq!(
            parse_str(&["--update", "--dry-run", "--yes"]).unwrap(),
            Command::Update {
                dry_run: true,
                assume_yes: true
            }
        );
        assert_eq!(
            parse_str(&["--update", "-y"]).unwrap(),
            Command::Update {
                dry_run: false,
                assume_yes: true
            }
        );
    }

    #[test]
    fn update_flags_do_nothing_on_their_own() {
        assert!(parse_str(&["--dry-run"]).is_err());
        assert!(parse_str(&["--yes"]).is_err());
        assert!(parse_str(&["-y"]).is_err());
    }
}
