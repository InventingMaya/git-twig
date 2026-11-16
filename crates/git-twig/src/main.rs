mod help;

use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if let Some(message) = help::for_args(&args) {
        print!("{message}");
        return ExitCode::SUCCESS;
    }
    if args.first().is_some_and(|arg| arg == "skill") {
        if args.len() != 1 {
            eprintln!("Usage: git twig skill");
            return ExitCode::FAILURE;
        }
        print!("{}", include_str!("../skills/git-twig/SKILL.md"));
        return ExitCode::SUCCESS;
    }
    if args.first().is_some_and(|arg| arg == "--version") {
        println!("git-twig {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    match env::current_dir()
        .map_err(anyhow::Error::from)
        .and_then(|cwd| git_twig::run(&cwd, &args))
    {
        Ok(code) => ExitCode::from(code as u8),
        Err(error) => {
            eprintln!("git-twig: {error:#}");
            ExitCode::FAILURE
        }
    }
}
