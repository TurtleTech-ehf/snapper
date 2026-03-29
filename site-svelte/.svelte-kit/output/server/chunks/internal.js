import { r as root } from "./root.js";
import "./environment.js";
let public_env = {};
function set_private_env(environment) {
}
function set_public_env(environment) {
  public_env = environment;
}
let read_implementation = null;
function set_read_implementation(fn) {
  read_implementation = fn;
}
function set_manifest(_) {
}
const options = {
  app_template_contains_nonce: false,
  async: false,
  csp: { "mode": "auto", "directives": { "upgrade-insecure-requests": false, "block-all-mixed-content": false }, "reportOnly": { "upgrade-insecure-requests": false, "block-all-mixed-content": false } },
  csrf_check_origin: true,
  csrf_trusted_origins: [],
  embedded: false,
  env_public_prefix: "PUBLIC_",
  env_private_prefix: "",
  hash_routing: false,
  hooks: null,
  // added lazily, via `get_hooks`
  preload_strategy: "modulepreload",
  root,
  service_worker: false,
  service_worker_options: void 0,
  server_error_boundaries: false,
  templates: {
    app: ({ head, body, assets, nonce, env }) => '<!DOCTYPE html>\n<html lang="en">\n<head>\n<meta charset="utf-8" />\n<meta name="viewport" content="width=device-width, initial-scale=1.0" />\n<title>snapper -- Semantic Line Breaks for Academic Writing</title>\n<meta name="description" content="A fast, format-aware semantic line break formatter for Org-mode, LaTeX, Markdown, and plaintext. Produces clean git diffs for collaborative academic writing.">\n<meta name="author" content="TurtleTech ehf">\n<meta name="keywords" content="semantic line breaks, formatter, LaTeX, Org-mode, Markdown, git diff, academic writing, sentence splitting, pre-commit">\n<link rel="canonical" href="https://snapper.turtletech.us/">\n<meta property="og:type" content="website">\n<meta property="og:url" content="https://snapper.turtletech.us/">\n<meta property="og:title" content="snapper - Semantic Line Breaks for Academic Writing">\n<meta property="og:description" content="A fast, format-aware semantic line break formatter. Produces clean git diffs for collaborative academic writing.">\n<meta property="og:image" content="https://snapper.turtletech.us/snapper_logo.png">\n<meta property="og:site_name" content="snapper">\n<meta name="twitter:card" content="summary_large_image">\n<meta name="twitter:title" content="snapper - Semantic Line Breaks">\n<meta name="twitter:description" content="Format prose so each sentence occupies its own line. Clean git diffs for collaborative writing.">\n<meta name="twitter:image" content="https://snapper.turtletech.us/snapper_logo.png">\n<link rel="icon" href="' + assets + '/favicon.ico" type="image/x-icon">\n<link rel="apple-touch-icon" href="' + assets + '/apple-touch-icon.png">\n<script defer data-website-id="af8c1286-06aa-414b-958c-e015bed1e569" src="https://analytics.turtletech.us/script.js"><\/script>\n<script type="application/ld+json">\n{\n  "@context": "https://schema.org",\n  "@type": "SoftwareApplication",\n  "name": "snapper",\n  "description": "Semantic line break formatter for Org-mode, LaTeX, Markdown, and plaintext",\n  "applicationCategory": "DeveloperApplication",\n  "operatingSystem": "Linux, macOS, Windows",\n  "url": "https://snapper.turtletech.us",\n  "downloadUrl": "https://crates.io/crates/snapper-fmt",\n  "softwareVersion": "0.3.2",\n  "license": "https://opensource.org/licenses/MIT",\n  "author": { "@type": "Organization", "name": "TurtleTech ehf", "url": "https://turtletech.us" },\n  "offers": { "@type": "Offer", "price": "0", "priceCurrency": "USD" }\n}\n<\/script>\n<link rel="preconnect" href="https://fonts.googleapis.com">\n<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>\n<link href="https://fonts.googleapis.com/css2?family=Jost:ital,wght@0,300;0,400;0,500;0,600;0,700;0,800;1,400&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet">\n' + head + '\n</head>\n<body data-sveltekit-preload-data="hover">\n' + body + "\n</body>\n</html>\n",
    error: ({ status, message }) => '<!doctype html>\n<html lang="en">\n	<head>\n		<meta charset="utf-8" />\n		<title>' + message + `</title>

		<style>
			body {
				--bg: white;
				--fg: #222;
				--divider: #ccc;
				background: var(--bg);
				color: var(--fg);
				font-family:
					system-ui,
					-apple-system,
					BlinkMacSystemFont,
					'Segoe UI',
					Roboto,
					Oxygen,
					Ubuntu,
					Cantarell,
					'Open Sans',
					'Helvetica Neue',
					sans-serif;
				display: flex;
				align-items: center;
				justify-content: center;
				height: 100vh;
				margin: 0;
			}

			.error {
				display: flex;
				align-items: center;
				max-width: 32rem;
				margin: 0 1rem;
			}

			.status {
				font-weight: 200;
				font-size: 3rem;
				line-height: 1;
				position: relative;
				top: -0.05rem;
			}

			.message {
				border-left: 1px solid var(--divider);
				padding: 0 0 0 1rem;
				margin: 0 0 0 1rem;
				min-height: 2.5rem;
				display: flex;
				align-items: center;
			}

			.message h1 {
				font-weight: 400;
				font-size: 1em;
				margin: 0;
			}

			@media (prefers-color-scheme: dark) {
				body {
					--bg: #222;
					--fg: #ddd;
					--divider: #666;
				}
			}
		</style>
	</head>
	<body>
		<div class="error">
			<span class="status">` + status + '</span>\n			<div class="message">\n				<h1>' + message + "</h1>\n			</div>\n		</div>\n	</body>\n</html>\n"
  },
  version_hash: "1dzwg6x"
};
async function get_hooks() {
  let handle;
  let handleFetch;
  let handleError;
  let handleValidationError;
  let init;
  let reroute;
  let transport;
  return {
    handle,
    handleFetch,
    handleError,
    handleValidationError,
    init,
    reroute,
    transport
  };
}
export {
  set_public_env as a,
  set_read_implementation as b,
  set_manifest as c,
  get_hooks as g,
  options as o,
  public_env as p,
  read_implementation as r,
  set_private_env as s
};
