const path = require("path");
const HtmlWebpackPlugin = require("html-webpack-plugin");
const CopyPlugin = require("copy-webpack-plugin");

module.exports = {
  entry: {
    taskpane: "./src/taskpane/taskpane.ts",
    commands: "./src/commands/commands.ts",
  },
  output: {
    path: path.resolve(__dirname, "dist"),
    filename: "[name].js",
    clean: true,
  },
  resolve: {
    extensions: [".ts", ".js", ".wasm"],
  },
  module: {
    rules: [
      {
        test: /\.ts$/,
        use: "ts-loader",
        exclude: /node_modules/,
      },
    ],
  },
  experiments: {
    asyncWebAssembly: true,
  },
  plugins: [
    new HtmlWebpackPlugin({
      template: "./src/taskpane/taskpane.html",
      filename: "taskpane.html",
      chunks: ["taskpane"],
    }),
    new CopyPlugin({
      patterns: [
        { from: "manifest.xml", to: "manifest.xml" },
        { from: "assets", to: "assets" },
      ],
    }),
  ],
  devServer: {
    port: 3000,
    https: true,
    headers: { "Access-Control-Allow-Origin": "*" },
  },
};
