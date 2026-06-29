use std::path::PathBuf;

use cbindgen::{Builder, Config, Language};
use clap::Parser;

/// Generate a C/C++/Cython header from a Rust source file or crate with cbindgen.
#[derive(Parser)]
#[command(about, long_about = None)]
struct Args {
    input: PathBuf,
    #[arg(short, long)]
    config: Option<PathBuf>,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(short, long)]
    lang: Option<String>,
}

fn parse_language(value: &str) -> Language {
    match value.to_lowercase().as_str() {
        "c" => Language::C,
        "c++" | "cpp" | "cxx" => Language::Cxx,
        "cython" => Language::Cython,
        other => panic!("unsupported language: {other} (expected c, c++, or cython)"),
    }
}

fn main() {
    let args = Args::parse();

    let config = match &args.config {
        Some(path) => Config::from_file(path).expect("cannot read cbindgen config"),
        None => Config::default(),
    };

    let mut builder = Builder::new().with_config(config);

    if args.input.is_file() {
        builder = builder.with_src(&args.input);
    } else {
        builder = builder.with_crate(&args.input);
    }

    if let Some(lang) = &args.lang {
        builder = builder.with_language(parse_language(lang));
    }

    let bindings = builder.generate().expect("failed to generate bindings");

    match &args.output {
        Some(path) => {
            bindings.write_to_file(path);
            println!("generated {}", path.display());
        }
        None => bindings.write(std::io::stdout()),
    }
}
