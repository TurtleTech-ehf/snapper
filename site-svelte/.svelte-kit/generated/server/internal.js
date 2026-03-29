
import root from '../root.js';
import { set_building, set_prerendering } from '__sveltekit/environment';
import { set_assets } from '$app/paths/internal/server';
import { set_manifest, set_read_implementation } from '__sveltekit/server';
import { set_private_env, set_public_env } from '../../../node_modules/@sveltejs/kit/src/runtime/shared-server.js';

export const options = {
	app_template_contains_nonce: false,
	async: false,
	csp: {"mode":"auto","directives":{"upgrade-insecure-requests":false,"block-all-mixed-content":false},"reportOnly":{"upgrade-insecure-requests":false,"block-all-mixed-content":false}},
	csrf_check_origin: true,
	csrf_trusted_origins: [],
	embedded: false,
	env_public_prefix: 'PUBLIC_',
	env_private_prefix: '',
	hash_routing: false,
	hooks: null, // added lazily, via `get_hooks`
	preload_strategy: "modulepreload",
	root,
	service_worker: false,
	service_worker_options: undefined,
	server_error_boundaries: false,
	templates: {
		app: ({ head, body, assets, nonce, env }) => "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\" />\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n<title>snapper -- Semantic Line Breaks for Academic Writing</title>\n<meta name=\"description\" content=\"A fast, format-aware semantic line break formatter for Org-mode, LaTeX, Markdown, and plaintext. Produces clean git diffs for collaborative academic writing.\">\n<meta name=\"author\" content=\"TurtleTech ehf\">\n<meta name=\"keywords\" content=\"semantic line breaks, formatter, LaTeX, Org-mode, Markdown, git diff, academic writing, sentence splitting, pre-commit\">\n<link rel=\"canonical\" href=\"https://snapper.turtletech.us/\">\n<meta property=\"og:type\" content=\"website\">\n<meta property=\"og:url\" content=\"https://snapper.turtletech.us/\">\n<meta property=\"og:title\" content=\"snapper - Semantic Line Breaks for Academic Writing\">\n<meta property=\"og:description\" content=\"A fast, format-aware semantic line break formatter. Produces clean git diffs for collaborative academic writing.\">\n<meta property=\"og:image\" content=\"https://snapper.turtletech.us/snapper_logo.png\">\n<meta property=\"og:site_name\" content=\"snapper\">\n<meta name=\"twitter:card\" content=\"summary_large_image\">\n<meta name=\"twitter:title\" content=\"snapper - Semantic Line Breaks\">\n<meta name=\"twitter:description\" content=\"Format prose so each sentence occupies its own line. Clean git diffs for collaborative writing.\">\n<meta name=\"twitter:image\" content=\"https://snapper.turtletech.us/snapper_logo.png\">\n<link rel=\"icon\" href=\"" + assets + "/favicon.ico\" type=\"image/x-icon\">\n<link rel=\"apple-touch-icon\" href=\"" + assets + "/apple-touch-icon.png\">\n<script defer data-website-id=\"af8c1286-06aa-414b-958c-e015bed1e569\" src=\"https://analytics.turtletech.us/script.js\"></script>\n<script type=\"application/ld+json\">\n{\n  \"@context\": \"https://schema.org\",\n  \"@type\": \"SoftwareApplication\",\n  \"name\": \"snapper\",\n  \"description\": \"Semantic line break formatter for Org-mode, LaTeX, Markdown, and plaintext\",\n  \"applicationCategory\": \"DeveloperApplication\",\n  \"operatingSystem\": \"Linux, macOS, Windows\",\n  \"url\": \"https://snapper.turtletech.us\",\n  \"downloadUrl\": \"https://crates.io/crates/snapper-fmt\",\n  \"softwareVersion\": \"0.3.2\",\n  \"license\": \"https://opensource.org/licenses/MIT\",\n  \"author\": { \"@type\": \"Organization\", \"name\": \"TurtleTech ehf\", \"url\": \"https://turtletech.us\" },\n  \"offers\": { \"@type\": \"Offer\", \"price\": \"0\", \"priceCurrency\": \"USD\" }\n}\n</script>\n<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">\n<link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin>\n<link href=\"https://fonts.googleapis.com/css2?family=Jost:ital,wght@0,300;0,400;0,500;0,600;0,700;0,800;1,400&family=JetBrains+Mono:wght@400;500;600&display=swap\" rel=\"stylesheet\">\n" + head + "\n</head>\n<body data-sveltekit-preload-data=\"hover\">\n" + body + "\n</body>\n</html>\n",
		error: ({ status, message }) => "<!doctype html>\n<html lang=\"en\">\n\t<head>\n\t\t<meta charset=\"utf-8\" />\n\t\t<title>" + message + "</title>\n\n\t\t<style>\n\t\t\tbody {\n\t\t\t\t--bg: white;\n\t\t\t\t--fg: #222;\n\t\t\t\t--divider: #ccc;\n\t\t\t\tbackground: var(--bg);\n\t\t\t\tcolor: var(--fg);\n\t\t\t\tfont-family:\n\t\t\t\t\tsystem-ui,\n\t\t\t\t\t-apple-system,\n\t\t\t\t\tBlinkMacSystemFont,\n\t\t\t\t\t'Segoe UI',\n\t\t\t\t\tRoboto,\n\t\t\t\t\tOxygen,\n\t\t\t\t\tUbuntu,\n\t\t\t\t\tCantarell,\n\t\t\t\t\t'Open Sans',\n\t\t\t\t\t'Helvetica Neue',\n\t\t\t\t\tsans-serif;\n\t\t\t\tdisplay: flex;\n\t\t\t\talign-items: center;\n\t\t\t\tjustify-content: center;\n\t\t\t\theight: 100vh;\n\t\t\t\tmargin: 0;\n\t\t\t}\n\n\t\t\t.error {\n\t\t\t\tdisplay: flex;\n\t\t\t\talign-items: center;\n\t\t\t\tmax-width: 32rem;\n\t\t\t\tmargin: 0 1rem;\n\t\t\t}\n\n\t\t\t.status {\n\t\t\t\tfont-weight: 200;\n\t\t\t\tfont-size: 3rem;\n\t\t\t\tline-height: 1;\n\t\t\t\tposition: relative;\n\t\t\t\ttop: -0.05rem;\n\t\t\t}\n\n\t\t\t.message {\n\t\t\t\tborder-left: 1px solid var(--divider);\n\t\t\t\tpadding: 0 0 0 1rem;\n\t\t\t\tmargin: 0 0 0 1rem;\n\t\t\t\tmin-height: 2.5rem;\n\t\t\t\tdisplay: flex;\n\t\t\t\talign-items: center;\n\t\t\t}\n\n\t\t\t.message h1 {\n\t\t\t\tfont-weight: 400;\n\t\t\t\tfont-size: 1em;\n\t\t\t\tmargin: 0;\n\t\t\t}\n\n\t\t\t@media (prefers-color-scheme: dark) {\n\t\t\t\tbody {\n\t\t\t\t\t--bg: #222;\n\t\t\t\t\t--fg: #ddd;\n\t\t\t\t\t--divider: #666;\n\t\t\t\t}\n\t\t\t}\n\t\t</style>\n\t</head>\n\t<body>\n\t\t<div class=\"error\">\n\t\t\t<span class=\"status\">" + status + "</span>\n\t\t\t<div class=\"message\">\n\t\t\t\t<h1>" + message + "</h1>\n\t\t\t</div>\n\t\t</div>\n\t</body>\n</html>\n"
	},
	version_hash: "1dzwg6x"
};

export async function get_hooks() {
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

export { set_assets, set_building, set_manifest, set_prerendering, set_private_env, set_public_env, set_read_implementation };
