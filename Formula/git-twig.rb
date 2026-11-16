class GitTwig < Formula
  desc "Lightweight Git worktrees using native copy-on-write files"
  homepage "https://github.com/leostera/git-twig"
  version "0.1.0"
  license "MIT"
  head "https://github.com/leostera/git-twig.git", branch: "main"

  depends_on "git"
  depends_on "rust" => :build if build.head?

  on_macos do
    on_arm do
      url "https://github.com/leostera/git-twig/releases/download/v0.1.0/git-twig-v0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "02762f8bd511231179785d4e248bb85446b2bf39a92e3ff618ba363602db2799"
    end
    on_intel do
      url "https://github.com/leostera/git-twig/releases/download/v0.1.0/git-twig-v0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "d70fd124a1510552c5450296c45bc643c5e71dc5b9b3c58f176be49b5beeafae"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/leostera/git-twig/releases/download/v0.1.0/git-twig-v0.1.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "f707d894a70c64a4cb7f7e1d1cf5aab88db10c38730195710eed77e1c2b68260"
    end
  end

  def install
    if build.head?
      system "cargo", "install", "--locked", "--path", "crates/git-twig", "--root", prefix
      man1.install "crates/git-twig/man/git-twig.1"
    else
      bin.install "git-twig"
      man1.install "git-twig.1"
    end
  end

  test do
    assert_match "git-twig", shell_output("#{bin}/git-twig --version")
  end
end

