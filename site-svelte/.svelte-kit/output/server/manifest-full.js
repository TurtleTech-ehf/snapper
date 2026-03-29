export const manifest = (() => {
function __memo(fn) {
	let value;
	return () => value ??= (value = fn());
}

return {
	appDir: "_app",
	appPath: "_app",
	assets: new Set(["apple-touch-icon.png","favicon.ico","robots.txt","sitemap.xml","snapper_logo.png"]),
	mimeTypes: {".png":"image/png",".txt":"text/plain",".xml":"text/xml"},
	_: {
		client: {start:"_app/immutable/entry/start.DSKOgrTV.js",app:"_app/immutable/entry/app.lokToFKc.js",imports:["_app/immutable/entry/start.DSKOgrTV.js","_app/immutable/chunks/DuopSPpD.js","_app/immutable/chunks/DZkGKIZp.js","_app/immutable/chunks/C3Ty10CT.js","_app/immutable/entry/app.lokToFKc.js","_app/immutable/chunks/DZkGKIZp.js","_app/immutable/chunks/BsWquQR9.js","_app/immutable/chunks/qjOqc3Sp.js","_app/immutable/chunks/C3Ty10CT.js","_app/immutable/chunks/Ch5-AlJj.js","_app/immutable/chunks/CBFXaQeh.js"],stylesheets:[],fonts:[],uses_env_dynamic_public:false},
		nodes: [
			__memo(() => import('./nodes/0.js')),
			__memo(() => import('./nodes/1.js')),
			__memo(() => import('./nodes/2.js'))
		],
		remotes: {
			
		},
		routes: [
			{
				id: "/",
				pattern: /^\/$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 2 },
				endpoint: null
			}
		],
		prerendered_routes: new Set([]),
		matchers: async () => {
			
			return {  };
		},
		server_assets: {}
	}
}
})();
