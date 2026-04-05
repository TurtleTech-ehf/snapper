#!/usr/bin/env node

// Postinstall: ensure the snapper binary is available.
// Priority: (1) already in PATH, (2) download pre-built from GitHub Releases,
// (3) cargo-binstall, (4) cargo install --features mcp.

const { execSync, spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");
const https = require("https");

const VERSION = require("../package.json").version;
const REPO = "TurtleTech-ehf/snapper";
const BASE_URL = `https://github.com/${REPO}/releases/download/v${VERSION}`;

function hasSnapper() {
  try {
    const v = execSync("snapper --version", { encoding: "utf-8", timeout: 5000 });
    return v.trim();
  } catch {
    return null;
  }
}

function getTarget() {
  const arch = os.arch();
  const platform = os.platform();

  const archMap = { x64: "x86_64", arm64: "aarch64" };
  const rustArch = archMap[arch];
  if (!rustArch) return null;

  switch (platform) {
    case "linux":
      return `${rustArch}-unknown-linux-gnu`;
    case "darwin":
      return `${rustArch}-apple-darwin`;
    case "win32":
      return arch === "x64" ? "x86_64-pc-windows-msvc" : null;
    default:
      return null;
  }
}

function download(url) {
  return new Promise((resolve, reject) => {
    const request = (u) => {
      https.get(u, { headers: { "User-Agent": "snapper-mcp-npm" } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          request(res.headers.location);
          return;
        }
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode} for ${u}`));
          return;
        }
        const chunks = [];
        res.on("data", (c) => chunks.push(c));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      }).on("error", reject);
    };
    request(url);
  });
}

async function installFromRelease() {
  const target = getTarget();
  if (!target) {
    console.log(`No pre-built binary for ${os.arch()}-${os.platform()}`);
    return false;
  }

  const isWindows = os.platform() === "win32";
  const ext = isWindows ? "zip" : "tar.xz";
  const assetName = `snapper-fmt-${target}.${ext}`;
  const url = `${BASE_URL}/${assetName}`;

  console.log(`Downloading ${url}`);

  let data;
  try {
    data = await download(url);
  } catch (e) {
    console.log(`Download failed: ${e.message}`);
    return false;
  }

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "snapper-"));
  const archivePath = path.join(tmpDir, assetName);
  fs.writeFileSync(archivePath, data);

  // Determine install directory
  const installDir = path.join(os.homedir(), ".local", "bin");
  if (!isWindows) {
    fs.mkdirSync(installDir, { recursive: true });
  }

  try {
    if (isWindows) {
      // Use PowerShell to extract zip
      spawnSync("powershell", [
        "-Command",
        `Expand-Archive -Path '${archivePath}' -DestinationPath '${tmpDir}' -Force`,
      ], { stdio: "inherit" });
      // Find the exe
      const exePath = findFile(tmpDir, "snapper.exe");
      if (exePath) {
        const dest = path.join(os.homedir(), ".local", "bin", "snapper.exe");
        fs.mkdirSync(path.dirname(dest), { recursive: true });
        fs.copyFileSync(exePath, dest);
        console.log(`Installed to ${dest}`);
        console.log("Ensure ~/.local/bin is in your PATH");
      }
    } else {
      // Use tar to extract
      const result = spawnSync("tar", ["xf", archivePath, "-C", tmpDir], {
        stdio: "inherit",
      });
      if (result.status !== 0) {
        console.log("tar extraction failed");
        return false;
      }
      // Find the binary inside the extracted directory
      const exePath = findFile(tmpDir, "snapper");
      if (exePath) {
        const dest = path.join(installDir, "snapper");
        fs.copyFileSync(exePath, dest);
        fs.chmodSync(dest, 0o755);
        console.log(`Installed to ${dest}`);
        // Check if installDir is in PATH
        if (!process.env.PATH.split(":").includes(installDir)) {
          console.log(`Add to PATH: export PATH="${installDir}:$PATH"`);
        }
      }
    }
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }

  return hasSnapper() !== null;
}

function findFile(dir, name) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      const found = findFile(full, name);
      if (found) return found;
    } else if (entry.name === name) {
      return full;
    }
  }
  return null;
}

function tryBinstall() {
  try {
    console.log("Trying: cargo binstall snapper-fmt");
    execSync("cargo binstall -y snapper-fmt", {
      stdio: "inherit",
      timeout: 120000,
    });
    return hasSnapper() !== null;
  } catch {
    return false;
  }
}

function tryCargoInstall() {
  try {
    console.log("Trying: cargo install snapper-fmt --features mcp");
    execSync("cargo install snapper-fmt --features mcp", {
      stdio: "inherit",
      timeout: 600000,
    });
    return hasSnapper() !== null;
  } catch {
    return false;
  }
}

async function main() {
  const version = hasSnapper();
  if (version) {
    console.log(`snapper found: ${version}`);
    return;
  }

  console.log("snapper binary not found. Attempting installation...\n");

  // 1. Pre-built binary from GitHub Releases
  if (await installFromRelease()) {
    console.log("Installed snapper from GitHub Releases.");
    return;
  }

  // 2. cargo-binstall
  if (tryBinstall()) {
    console.log("Installed snapper via cargo-binstall.");
    return;
  }

  // 3. cargo install with MCP feature
  if (tryCargoInstall()) {
    console.log("Installed snapper with MCP support via cargo install.");
    return;
  }

  console.error(
    "\nCould not install snapper automatically.\n" +
      "Install manually:\n" +
      `  curl -LsSf ${BASE_URL}/snapper-fmt-installer.sh | sh\n` +
      "  cargo binstall snapper-fmt\n" +
      "  cargo install snapper-fmt --features mcp\n" +
      "  pip install snapper-fmt\n" +
      "  brew install TurtleTech-ehf/tap/snapper-fmt\n\n" +
      "See https://snapper.turtletech.us for all options.\n",
  );
}

main().catch((e) => {
  console.error("postinstall error:", e.message);
  // Don't fail npm install
  process.exit(0);
});
