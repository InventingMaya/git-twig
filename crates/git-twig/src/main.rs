use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
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
    if args.is_empty() || args.first().is_some_and(|arg| arg == "--help") {
        println!(
            "git twig — lightweight Git worktrees\n\nUsage: git twig <worktree arguments>\n       git twig skill  (print the bundled agent skill)\n\nFast path: add [-b <new-branch> | --detach] <path> [HEAD]\nOther operations and unsupported cases use git worktree unchanged.\nRequires Git 2.40+. Native CoW supports macOS/APFS and Linux reflink filesystems.\nSet TWIG_TRACE=1 to report backend selection to stderr."
        );
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
