#!/usr/bin/env node

const { spawn } = require("child_process");
const { execSync } = require("child_process");

let snapperBinary;

try {
  const result = execSync("which snapper", { encoding: "utf-8" });
  snapperBinary = result.trim();
} catch {
  console.error(
    "snapper binary not found in PATH.\n" +
      "Install: cargo binstall snapper-fmt  (or see https://snapper.turtletech.us)",
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
