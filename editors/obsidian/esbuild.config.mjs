import esbuild from "esbuild";
import { readFileSync } from "fs";

const watch = process.argv.includes("--watch");

/** Inline .wasm files as base64 data URLs. */
const wasmPlugin = {
  name: "wasm-inline",
  setup(build) {
    build.onLoad({ filter: /\.wasm$/ }, (args) => {
      const bytes = readFileSync(args.path);
      const base64 = bytes.toString("base64");
      return {
        contents: `export default Uint8Array.from(atob("${base64}"), c => c.charCodeAt(0)).buffer;`,
        loader: "js",
      };
    });
  },
};

const ctx = await esbuild.context({
  entryPoints: ["src/main.ts"],
  bundle: true,
  outfile: "main.js",
  platform: "node",
  external: ["obsidian"],
  format: "cjs",
  sourcemap: watch ? "inline" : false,
  plugins: [wasmPlugin],
  logLevel: "info",
});

if (watch) {
  await ctx.watch();
} else {
  await ctx.rebuild();
  await ctx.dispose();
}
