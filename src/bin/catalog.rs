//! Prints the level catalog as Markdown: `cargo run --release --bin catalog > docs/LEVEL_CATALOG.md`
fn main() {
    print!("{}", vfield::catalog::render());
}
