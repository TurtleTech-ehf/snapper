#!/usr/bin/env node

const { spawn, execSync } = require("child_process");

const isWindows = process.platform === "win32";
const binaryName = isWindows ? "snapper.exe" : "snapper";

let snapperBinary;

try {
  const cmd = isWindows ? "where" : "which";
  const result = execSync(`${cmd} ${binaryName}`, { encoding: "utf-8" });
  snapperBinary = result.trim().split("\n")[0];
} catch {
  console.error(
    "snapper binary not found in PATH.\n\n" +
      "Install one of:\n" +
      "  cargo binstall snapper-fmt\n" +
      "  cargo install snapper-fmt --features mcp\n" +
      "  pip install snapper-fmt\n" +
      "  brew install TurtleTech-ehf/tap/snapper-fmt\n\n" +
      "See https://snapper.turtletech.us for all options.",
  );
  process.exit(1);
}

const child = spawn(snapperBinary, ["mcp"], {
  stdio: ["pipe", "pipe", "inherit"],
});

process.stdin.pipe(child.stdin);
child.stdout.pipe(process.stdout);

child.on("error", (err) => {
  console.error("Error running snapper mcp:", err.message);
  process.exit(1);
});

child.on("exit", (code) => {
  process.exit(code || 0);
});
