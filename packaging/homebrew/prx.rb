class Prx < Formula
  desc "Agent-native Unix tools for AI coding agents"
  homepage "https://github.com/civitas-io/prx"
  version "0.5.11"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/civitas-io/prx/releases/download/v0.5.11/prx-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_WILL_UPDATE_ON_RELEASE"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/civitas-io/prx/releases/download/v0.5.11/prx-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_WILL_UPDATE_ON_RELEASE"
    end
    on_arm do
      url "https://github.com/civitas-io/prx/releases/download/v0.5.11/prx-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_WILL_UPDATE_ON_RELEASE"
    end
  end

  def install
    bin.install "prx"
  end

  test do
    assert_match "prx", shell_output("#{bin}/prx --version")
  end
end
