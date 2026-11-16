class GitTwig < Formula
  desc "Lightweight Git worktrees using native copy-on-write files"
  homepage "https://github.com/leostera/git-twig"
  version "0.3.0"
  license "MIT"
  head "https://github.com/leostera/git-twig.git", branch: "main"

  depends_on "git"
  depends_on "rust" => :build if build.head?

  on_macos do
    on_arm do
      url "https://github.com/leostera/git-twig/releases/download/v0.3.0/git-twig-v0.3.0-aarch64-apple-darwin.tar.gz"
      sha256 "0159a41f37dd53e5b09310ca64eb2e1749a298e87b648ded4514d2013853cbaf"
    end
    on_intel do
      url "https://github.com/leostera/git-twig/releases/download/v0.3.0/git-twig-v0.3.0-x86_64-apple-darwin.tar.gz"
      sha256 "d35dd112063e91cf40d2c74b95b278888fba017d1838d0bda57d946a1868b2f6"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/leostera/git-twig/releases/download/v0.3.0/git-twig-v0.3.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "786386e0c3102043dc9efbb6b6e736a1f6ad287507a462d136871bf73c3d5e13"
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

