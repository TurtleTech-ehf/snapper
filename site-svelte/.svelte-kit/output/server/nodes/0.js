import * as universal from '../entries/pages/_layout.ts.js';

export const index = 0;
let component_cache;
export const component = async () => component_cache ??= (await import('../entries/pages/_layout.svelte.js')).default;
export { universal };
export const universal_id = "src/routes/+layout.ts";
export const imports = ["_app/immutable/nodes/0.D9d642DQ.js","_app/immutable/chunks/qjOqc3Sp.js","_app/immutable/chunks/DZkGKIZp.js","_app/immutable/chunks/jEPLLRAN.js","_app/immutable/chunks/CBFXaQeh.js"];
export const stylesheets = ["_app/immutable/assets/0.s-PLTMji.css"];
export const fonts = [];
