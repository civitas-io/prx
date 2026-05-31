class Prx < Formula
  desc "Agent-native Unix tools for AI coding agents"
  homepage "https://github.com/civitas-io/prx"
  version "0.5.9"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/civitas-io/prx/releases/download/v0.5.9/prx-aarch64-apple-darwin.tar.gz"
      sha256 "1c8ef432f6730a6e29cd42e8a11161282e198634129c4230d03baf90d02e0290"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/civitas-io/prx/releases/download/v0.5.9/prx-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "b8faea1409e6b0523a20bd0d8a02769365a35cfd762544b1d872b5703d429a40"
    end
    on_arm do
      url "https://github.com/civitas-io/prx/releases/download/v0.5.9/prx-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "0693dc70d08880f2e8b0a60cd8368aa1ed64b1ea40b5f49d0337abc5f5db55ac"
    end
  end

  def install
    bin.install "prx"
  end

  test do
    assert_match "prx", shell_output("#{bin}/prx --version")
  end
end
