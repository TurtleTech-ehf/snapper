import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  kit: {
    adapter: adapter({
      pages: 'build',
      assets: 'build',
      fallback: undefined,
      precompress: false,
      strict: true,
    }),
    prerender: {
      handleHttpError: ({ path }) => {
        // docs/ is served by Sphinx, not SvelteKit
        if (path.startsWith('/docs')) return;
        throw new Error(`404: ${path}`);
      },
    },
  },
};

export default config;
