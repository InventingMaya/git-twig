# Releases

Nothing is automatically published to crates.io. No release has been made yet.

Before publishing:
1. Run all local/CI checks and benchmark the intended commit.
2. Confirm both crate names are available on crates.io.
3. Review cargo package --list for each public crate; never include local fixtures.
4. Bump the shared workspace version and twig-core dependency version together.
5. Configure GitHub's **release** environment with required reviewers. Ensure the
   release bot may update Formula/git-twig.rb on main, or adapt it to open a PR.
6. Review GitHub Actions permissions and the generated formula.

With owner approval, push a vVERSION tag matching Cargo.toml. The release workflow
builds macOS arm64/x86_64 and Linux x86_64 archives, SHA256SUMS and a stable
Homebrew formula. It creates a GitHub release and updates the same repository's
Formula/git-twig.rb. Linux currently delegates to Git; it is not a native reflink
backend. macOS binaries are not signed/notarized yet.

Homebrew needs the explicit remote because the repository is not named
homebrew-git-twig:

```sh
brew tap leostera/git-twig https://github.com/leostera/git-twig
brew install leostera/git-twig/git-twig
```

Before the first release, use --HEAD instead.

Crates.io publishing is a separate owner-approved action:
publish twig-core first; wait for the registry index to expose that version;
then publish git-twig. Use cargo login locally, never paste credentials into chat.
Do not publish twig-bench. The CLI's local path dependency also has a registry
version, so cargo installs from crates.io resolve the published core.

Workflow YAML and formula generation can be checked locally; actual hosted builds,
downloaded binary installs and registry publication still require end-to-end
validation after repository publication.
