class GitTwig < Formula
  desc "Lightweight Git worktrees using native copy-on-write files"
  homepage "https://github.com/leostera/git-twig"
  version "0.2.0"
  license "MIT"
  head "https://github.com/leostera/git-twig.git", branch: "main"

  depends_on "git"
  depends_on "rust" => :build if build.head?

  on_macos do
    on_arm do
      url "https://github.com/leostera/git-twig/releases/download/v0.2.0/git-twig-v0.2.0-aarch64-apple-darwin.tar.gz"
      sha256 "d355bef7aa983f738b4e0b83e73e5786db4bc456279597128d4e21bfdef25b86"
    end
    on_intel do
      url "https://github.com/leostera/git-twig/releases/download/v0.2.0/git-twig-v0.2.0-x86_64-apple-darwin.tar.gz"
      sha256 "14dd30e3964e6b72053a57034c77d1b01964c57e3d3917c63485b89d94bd9908"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/leostera/git-twig/releases/download/v0.2.0/git-twig-v0.2.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "0a90d39499d65e8274b44c519fbcaec9ee6a323ad0574ccdab02400715c04ddd"
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

