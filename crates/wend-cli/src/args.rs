use anyhow::{Result, bail};

const HELP: &str = "usage: wend [file/to/open]";

pub(crate) struct Args {
    pub(crate) file: String,
}

pub(crate) fn parse_args() -> Result<Args> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut parser = lexopt::Parser::from_env();
    while let Some(argument) = parser.next()? {
        match argument {
            Value(value) if file.is_none() => {
                file = Some(value.string()?);
            }
            Long("help") => {
                bail!(HELP);
            }
            _ => return Err(argument.unexpected().into()),
        }
    }

    let Some(file) = file else { bail!(HELP) };

    Ok(Args { file })
}
