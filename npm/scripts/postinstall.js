#!/usr/bin/env node

// Postinstall: ensure the snapper binary is available with MCP support.
// Tries (in order): check PATH, cargo-binstall, cargo install.

const { execSync } = require("child_process");

function hasSnapper() {
  try {
    const version = execSync("snapper --version", { encoding: "utf-8" });
    return version.trim();
  } catch {
    return null;
  }
}

function hasSnapperMcp() {
  try {
    execSync("snapper mcp --help", {
      encoding: "utf-8",
      timeout: 5000,
    });
    return true;
  } catch {
    return false;
  }
}

const version = hasSnapper();
if (version) {
  console.log(`snapper found: ${version}`);
  // Can't easily verify MCP feature from outside, but the binary exists.
  // If `snapper mcp` fails at runtime it will print a clear error.
  process.exit(0);
}

console.log("snapper binary not found. Attempting installation...");

// Try cargo-binstall (fast, pre-built)
try {
  console.log("Trying: cargo binstall snapper-fmt");
  execSync("cargo binstall -y snapper-fmt", {
    stdio: "inherit",
    timeout: 120000,
  });
  if (hasSnapper()) {
    console.log("Installed snapper via cargo-binstall.");
    console.log(
      "Note: pre-built binaries may not include the MCP feature.",
    );
    console.log(
      "If `snapper mcp` fails, rebuild with: cargo install snapper-fmt --features mcp",
    );
    process.exit(0);
  }
} catch {
  // cargo-binstall not available or failed
}

// Try cargo install with MCP feature
try {
  console.log("Trying: cargo install snapper-fmt --features mcp");
  execSync("cargo install snapper-fmt --features mcp", {
    stdio: "inherit",
    timeout: 600000,
  });
  if (hasSnapper()) {
    console.log("Installed snapper with MCP support via cargo install.");
    process.exit(0);
  }
} catch {
  // cargo not available
}

console.error(
  "\nCould not install snapper automatically.\n" +
    "Install manually:\n" +
    "  cargo binstall snapper-fmt              # pre-built binary\n" +
    "  cargo install snapper-fmt --features mcp  # from source with MCP\n" +
    "  pip install snapper-fmt                 # via PyPI\n" +
    "  brew install TurtleTech-ehf/tap/snapper-fmt  # Homebrew\n" +
    "\nSee https://snapper.turtletech.us for all options.\n",
);
// Don't fail npm install -- the user can install snapper separately
process.exit(0);
