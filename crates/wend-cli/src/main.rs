//! The `wend` binary: the terminal frontend for `wend-core`.

struct Args {
    file: String,
}

fn parse_args() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Value(val) if file.is_none() => {
                file = Some(val.string()?);
            }
            Long("help") => {
                println!("Usage: hello [-n|--number=NUM] [--shout] THING");
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Args {
        file: file.unwrap(),
    })
}

#[expect(
    clippy::print_stdout,
    reason = "placeholder until the terminal frontend exists"
)]
fn main() -> Result<(), lexopt::Error> {
    let args = parse_args()?;



    Ok(())
}
