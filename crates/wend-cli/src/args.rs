use anyhow::{Result, bail};

const HELP: &str =
    "usage: wend [file/to/open]\n       wend --extend-config\n       wend --reset-config";

/// What the command line asks wend to do.
pub(crate) enum Command {
    /// Edit the file at this path.
    Edit(String),
    /// Add every setting the config file lacks.
    ExtendConfig,
    /// Replace the config file with the default config.
    ResetConfig,
}

pub(crate) fn parse_args() -> Result<Command> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut config_command = None;
    let mut parser = lexopt::Parser::from_env();
    while let Some(argument) = parser.next()? {
        match argument {
            Value(value) if file.is_none() => {
                file = Some(value.string()?);
            }
            Long("extend-config") => {
                if config_command.replace(Command::ExtendConfig).is_some() {
                    bail!(HELP);
                }
            }
            Long("reset-config") => {
                if config_command.replace(Command::ResetConfig).is_some() {
                    bail!(HELP);
                }
            }
            Long("help") => {
                bail!(HELP);
            }
            _ => return Err(argument.unexpected().into()),
        }
    }

    match (file, config_command) {
        (None, Some(command)) => Ok(command),
        (Some(file), None) => Ok(Command::Edit(file)),
        _ => bail!(HELP),
    }
}
