import esbuild from "esbuild";

const watch = process.argv.includes("--watch");

const ctx = await esbuild.context({
    entryPoints: ["src/extension.ts"],
    bundle: true,
    outfile: "out/extension.js",
    platform: "node",
    external: ["vscode"],
    format: "cjs",
    sourcemap: true,
    logLevel: "info",
});

if (watch) {
    await ctx.watch();
} else {
    await ctx.rebuild();
    await ctx.dispose();
}
