class Mosaic < Formula
  desc "Mosaic local-agent CLI"
  homepage "https://github.com/ooiai/mosaic"
  url "https://github.com/ooiai/mosaic/archive/refs/tags/v0.2.0-beta.5.tar.gz"
  sha256 "61b14f05c9cad39f68b11fc64c71a60a7e50928cfea3e6ec7c9c61fd3fdff536"
  license "MIT"
  head "https://github.com/ooiai/mosaic.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install",
           "--locked",
           "--path", "cli/crates/mosaic-cli",
           "--root", prefix
  end

  test do
    assert_match "mosaic", shell_output("#{bin}/mosaic --help")
  end
end
