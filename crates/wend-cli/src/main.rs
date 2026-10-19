//! The `wend` binary: the terminal frontend for `wend-core`.

#[expect(
    clippy::print_stdout,
    reason = "placeholder until the terminal frontend exists"
)]
fn main() {
    println!("wend {} is under construction", wend_core::VERSION);
}
