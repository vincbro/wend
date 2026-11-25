use anyhow::{Result, bail};

const HELP: &str =
    "usage: wend [file/to/open]\n       wend --default-config\n       wend --theme <name>";

/// What the command line asks wend to do.
pub(crate) enum Command {
    /// Edit the file at this path, or an unnamed buffer when there is none.
    Edit(Option<String>),
    /// Print the default config.
    DefaultConfig,
    /// Print the built-in theme with this name.
    Theme(Box<str>),
}

pub(crate) fn parse_args() -> Result<Command> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut command = None;
    let mut parser = lexopt::Parser::from_env();
    while let Some(argument) = parser.next()? {
        match argument {
            Value(value) if file.is_none() => {
                file = Some(value.string()?);
            }
            Long("default-config") => {
                if command.replace(Command::DefaultConfig).is_some() {
                    bail!(HELP);
                }
            }
            Long("theme") => {
                if command
                    .replace(Command::Theme(parser.value()?.string()?.into()))
                    .is_some()
                {
                    bail!(HELP);
                }
            }
            Long("help") => {
                bail!(HELP);
            }
            _ => return Err(argument.unexpected().into()),
        }
    }

    match (file, command) {
        (None, Some(command)) => Ok(command),
        (file, None) => Ok(Command::Edit(file)),
        (Some(_), Some(_)) => bail!(HELP),
    }
}
