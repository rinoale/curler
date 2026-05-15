use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    Run(Vec<String>),
    Help,
    Version,
}

pub fn cli_action(args: &[String]) -> CliAction {
    match args {
        [arg] if arg == "--help" || arg == "-h" => CliAction::Help,
        [arg] if arg == "--version" || arg == "-v" => CliAction::Version,
        _ => CliAction::Run(args.to_vec()),
    }
}

pub fn handle_cli_action(action: CliAction) -> io::Result<()> {
    match action {
        CliAction::Help => {
            print!("{}", help_text());
            Ok(())
        }
        CliAction::Version => {
            println!("curler {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        CliAction::Run(_) => Ok(()),
    }
}

fn help_text() -> String {
    format!(
        "\
curler {version}
Terminal HTTP client

USAGE:
    curler
    curler <url> [curl-compatible-options]

OPTIONS:
    -h, --help       Print help
    -v, --version    Print version

NOTES:
    Curler options are recognized only when they are the sole argument.
    Extra arguments are imported as request arguments.
",
        version = env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn cli_flags_work_when_they_are_the_only_argument() {
        assert_eq!(cli_action(&strings(&["--help"])), CliAction::Help);
        assert_eq!(cli_action(&strings(&["-h"])), CliAction::Help);
        assert_eq!(cli_action(&strings(&["--version"])), CliAction::Version);
        assert_eq!(cli_action(&strings(&["-v"])), CliAction::Version);
    }

    #[test]
    fn cli_flags_are_import_args_when_part_of_a_request() {
        assert_eq!(
            cli_action(&strings(&["-v", "https://example.com"])),
            CliAction::Run(strings(&["-v", "https://example.com"]))
        );
        assert_eq!(
            cli_action(&strings(&["https://example.com", "-h"])),
            CliAction::Run(strings(&["https://example.com", "-h"]))
        );
    }

    #[test]
    fn help_text_mentions_supported_flags() {
        let text = help_text();

        assert!(text.contains("--help"));
        assert!(text.contains("--version"));
        assert!(text.contains("curler <url> [curl-compatible-options]"));
        assert!(!text.contains("curler curl"));
    }
}
