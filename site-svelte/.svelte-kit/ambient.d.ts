
// this file is generated — do not edit it


/// <reference types="@sveltejs/kit" />

/**
 * This module provides access to environment variables that are injected _statically_ into your bundle at build time and are limited to _private_ access.
 * 
 * |         | Runtime                                                                    | Build time                                                               |
 * | ------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
 * | Private | [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private) | [`$env/static/private`](https://svelte.dev/docs/kit/$env-static-private) |
 * | Public  | [`$env/dynamic/public`](https://svelte.dev/docs/kit/$env-dynamic-public)   | [`$env/static/public`](https://svelte.dev/docs/kit/$env-static-public)   |
 * 
 * Static environment variables are [loaded by Vite](https://vitejs.dev/guide/env-and-mode.html#env-files) from `.env` files and `process.env` at build time and then statically injected into your bundle at build time, enabling optimisations like dead code elimination.
 * 
 * **_Private_ access:**
 * 
 * - This module cannot be imported into client-side code
 * - This module only includes variables that _do not_ begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) _and do_ start with [`config.kit.env.privatePrefix`](https://svelte.dev/docs/kit/configuration#env) (if configured)
 * 
 * For example, given the following build time environment:
 * 
 * ```env
 * ENVIRONMENT=production
 * PUBLIC_BASE_URL=http://site.com
 * ```
 * 
 * With the default `publicPrefix` and `privatePrefix`:
 * 
 * ```ts
 * import { ENVIRONMENT, PUBLIC_BASE_URL } from '$env/static/private';
 * 
 * console.log(ENVIRONMENT); // => "production"
 * console.log(PUBLIC_BASE_URL); // => throws error during build
 * ```
 * 
 * The above values will be the same _even if_ different values for `ENVIRONMENT` or `PUBLIC_BASE_URL` are set at runtime, as they are statically replaced in your code with their build time values.
 */
declare module '$env/static/private' {
	export const SHELL: string;
	export const npm_command: string;
	export const LSCOLORS: string;
	export const COREPACK_ENABLE_AUTO_PIN: string;
	export const npm_config_userconfig: string;
	export const COLORTERM: string;
	export const npm_config_cache: string;
	export const LESS: string;
	export const WORDCHARS: string;
	export const LESSCHARSET: string;
	export const TERM_PROGRAM_VERSION: string;
	export const TMUX: string;
	export const LANGUAGE: string;
	export const NODE: string;
	export const LESS_TERMCAP_se: string;
	export const LESS_TERMCAP_so: string;
	export const SSH_AUTH_SOCK: string;
	export const PIXI_PROMPT: string;
	export const MEMORY_PRESSURE_WRITE: string;
	export const TMUX_PLUGIN_MANAGER_PATH: string;
	export const RIPGREP_CONFIG_PATH: string;
	export const COLOR: string;
	export const npm_config_local_prefix: string;
	export const __ETC_PROFILE_NIX_SOURCED: string;
	export const npm_config_globalconfig: string;
	export const GPG_TTY: string;
	export const XML_CATALOG_FILES: string;
	export const PUPPETEER_EXECUTABLE_PATH: string;
	export const EDITOR: string;
	export const GOBIN: string;
	export const PWD: string;
	export const NIX_PROFILES: string;
	export const GSETTINGS_SCHEMA_DIR: string;
	export const LOGNAME: string;
	export const CONDA_PREFIX: string;
	export const ___X_CMD_LOG_C_INFO: string;
	export const npm_config_init_module: string;
	export const SYSTEMD_EXEC_PID: string;
	export const GSETTINGS_SCHEMA_DIR_CONDA_BACKUP: string;
	export const PIXI_PROJECT_MANIFEST: string;
	export const PIXI_PROJECT_NAME: string;
	export const _: string;
	export const NoDefaultCurrentDirectoryInExePath: string;
	export const FZF_DEFAULT_COMMAND: string;
	export const CLAUDECODE: string;
	export const GPU_LLM_DIR: string;
	export const HOME: string;
	export const SSH_ASKPASS: string;
	export const LANG: string;
	export const LS_COLORS: string;
	export const npm_package_version: string;
	export const MEMORY_PRESSURE_WATCH: string;
	export const STARSHIP_SHELL: string;
	export const STARSHIP_CONFIG: string;
	export const NIX_SSL_CERT_FILE: string;
	export const PIXI_ENVIRONMENT_NAME: string;
	export const PERL5LIB: string;
	export const _ble_util_fd_zero: string;
	export const INVOCATION_ID: string;
	export const ___X_CMD_LOG_C_DEBUG: string;
	export const MANAGERPID: string;
	export const PUPPETEER_CACHE_DIR: string;
	export const PIXI_IN_SHELL: string;
	export const BASH_SILENCE_DEPRECATION_WARNING: string;
	export const INIT_CWD: string;
	export const STARSHIP_SESSION_KEY: string;
	export const CORRECT_IGNORE_FILE: string;
	export const npm_lifecycle_script: string;
	export const CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: string;
	export const ___X_CMD_LOG_C_TF: string;
	export const npm_config_npm_version: string;
	export const _ble_util_fd_stdin: string;
	export const UV_INDEX_STRATEGY: string;
	export const PIXI_EXE: string;
	export const TERM: string;
	export const npm_package_name: string;
	export const LESS_TERMCAP_mb: string;
	export const LESS_TERMCAP_me: string;
	export const LESS_TERMCAP_md: string;
	export const PERL_MB_OPT: string;
	export const npm_config_prefix: string;
	export const CORRECT_IGNORE: string;
	export const CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS: string;
	export const USER: string;
	export const TMUX_PANE: string;
	export const ___X_CMD_LOG_C_ERROR: string;
	export const CONDA_SHLVL: string;
	export const PERL_MM_OPT: string;
	export const DISPLAY: string;
	export const npm_lifecycle_event: string;
	export const SHLVL: string;
	export const LESS_TERMCAP_ue: string;
	export const _ble_util_fd_stderr: string;
	export const LESS_TERMCAP_us: string;
	export const GIT_EDITOR: string;
	export const PAGER: string;
	export const PIXI_PROJECT_VERSION: string;
	export const GOCACHE: string;
	export const ATUIN_SESSION: string;
	export const MANAGERPIDFDID: string;
	export const npm_config_user_agent: string;
	export const OTEL_EXPORTER_OTLP_METRICS_TEMPORALITY_PREFERENCE: string;
	export const _ble_util_fd_null: string;
	export const DEEPSEEK_API_KEY: string;
	export const userid: string;
	export const npm_execpath: string;
	export const PIXI_PROJECT_ROOT: string;
	export const ATUIN_HISTORY_ID: string;
	export const LC_CTYPE: string;
	export const XDG_RUNTIME_DIR: string;
	export const CONDA_DEFAULT_ENV: string;
	export const PIXI_ENVIRONMENT_PLATFORMS: string;
	export const CLAUDE_CODE_ENTRYPOINT: string;
	export const DEBUGINFOD_URLS: string;
	export const npm_package_json: string;
	export const LC_TIME: string;
	export const DOCKER_HOST: string;
	export const LC_ALL: string;
	export const ___X_CMD_LOG_C_WARN: string;
	export const JOURNAL_STREAM: string;
	export const XDG_DATA_DIRS: string;
	export const PERL_LOCAL_LIB_ROOT: string;
	export const shellHome: string;
	export const npm_config_noproxy: string;
	export const PATH: string;
	export const npm_config_node_gyp: string;
	export const groupid: string;
	export const DBUS_SESSION_BUS_ADDRESS: string;
	export const FZF_DEFAULT_OPTS: string;
	export const npm_config_global_prefix: string;
	export const HG: string;
	export const npm_node_execpath: string;
	export const PUPPETEER_SKIP_CHROMIUM_DOWNLOAD: string;
	export const OLDPWD: string;
	export const GOPATH: string;
	export const TERM_PROGRAM: string;
	export const _ble_util_fd_stdout: string;
	export const NODE_ENV: string;
}

/**
 * This module provides access to environment variables that are injected _statically_ into your bundle at build time and are _publicly_ accessible.
 * 
 * |         | Runtime                                                                    | Build time                                                               |
 * | ------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
 * | Private | [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private) | [`$env/static/private`](https://svelte.dev/docs/kit/$env-static-private) |
 * | Public  | [`$env/dynamic/public`](https://svelte.dev/docs/kit/$env-dynamic-public)   | [`$env/static/public`](https://svelte.dev/docs/kit/$env-static-public)   |
 * 
 * Static environment variables are [loaded by Vite](https://vitejs.dev/guide/env-and-mode.html#env-files) from `.env` files and `process.env` at build time and then statically injected into your bundle at build time, enabling optimisations like dead code elimination.
 * 
 * **_Public_ access:**
 * 
 * - This module _can_ be imported into client-side code
 * - **Only** variables that begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) (which defaults to `PUBLIC_`) are included
 * 
 * For example, given the following build time environment:
 * 
 * ```env
 * ENVIRONMENT=production
 * PUBLIC_BASE_URL=http://site.com
 * ```
 * 
 * With the default `publicPrefix` and `privatePrefix`:
 * 
 * ```ts
 * import { ENVIRONMENT, PUBLIC_BASE_URL } from '$env/static/public';
 * 
 * console.log(ENVIRONMENT); // => throws error during build
 * console.log(PUBLIC_BASE_URL); // => "http://site.com"
 * ```
 * 
 * The above values will be the same _even if_ different values for `ENVIRONMENT` or `PUBLIC_BASE_URL` are set at runtime, as they are statically replaced in your code with their build time values.
 */
declare module '$env/static/public' {
	
}

/**
 * This module provides access to environment variables set _dynamically_ at runtime and that are limited to _private_ access.
 * 
 * |         | Runtime                                                                    | Build time                                                               |
 * | ------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
 * | Private | [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private) | [`$env/static/private`](https://svelte.dev/docs/kit/$env-static-private) |
 * | Public  | [`$env/dynamic/public`](https://svelte.dev/docs/kit/$env-dynamic-public)   | [`$env/static/public`](https://svelte.dev/docs/kit/$env-static-public)   |
 * 
 * Dynamic environment variables are defined by the platform you're running on. For example if you're using [`adapter-node`](https://github.com/sveltejs/kit/tree/main/packages/adapter-node) (or running [`vite preview`](https://svelte.dev/docs/kit/cli)), this is equivalent to `process.env`.
 * 
 * **_Private_ access:**
 * 
 * - This module cannot be imported into client-side code
 * - This module includes variables that _do not_ begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) _and do_ start with [`config.kit.env.privatePrefix`](https://svelte.dev/docs/kit/configuration#env) (if configured)
 * 
 * > [!NOTE] In `dev`, `$env/dynamic` includes environment variables from `.env`. In `prod`, this behavior will depend on your adapter.
 * 
 * > [!NOTE] To get correct types, environment variables referenced in your code should be declared (for example in an `.env` file), even if they don't have a value until the app is deployed:
 * >
 * > ```env
 * > MY_FEATURE_FLAG=
 * > ```
 * >
 * > You can override `.env` values from the command line like so:
 * >
 * > ```sh
 * > MY_FEATURE_FLAG="enabled" npm run dev
 * > ```
 * 
 * For example, given the following runtime environment:
 * 
 * ```env
 * ENVIRONMENT=production
 * PUBLIC_BASE_URL=http://site.com
 * ```
 * 
 * With the default `publicPrefix` and `privatePrefix`:
 * 
 * ```ts
 * import { env } from '$env/dynamic/private';
 * 
 * console.log(env.ENVIRONMENT); // => "production"
 * console.log(env.PUBLIC_BASE_URL); // => undefined
 * ```
 */
declare module '$env/dynamic/private' {
	export const env: {
		SHELL: string;
		npm_command: string;
		LSCOLORS: string;
		COREPACK_ENABLE_AUTO_PIN: string;
		npm_config_userconfig: string;
		COLORTERM: string;
		npm_config_cache: string;
		LESS: string;
		WORDCHARS: string;
		LESSCHARSET: string;
		TERM_PROGRAM_VERSION: string;
		TMUX: string;
		LANGUAGE: string;
		NODE: string;
		LESS_TERMCAP_se: string;
		LESS_TERMCAP_so: string;
		SSH_AUTH_SOCK: string;
		PIXI_PROMPT: string;
		MEMORY_PRESSURE_WRITE: string;
		TMUX_PLUGIN_MANAGER_PATH: string;
		RIPGREP_CONFIG_PATH: string;
		COLOR: string;
		npm_config_local_prefix: string;
		__ETC_PROFILE_NIX_SOURCED: string;
		npm_config_globalconfig: string;
		GPG_TTY: string;
		XML_CATALOG_FILES: string;
		PUPPETEER_EXECUTABLE_PATH: string;
		EDITOR: string;
		GOBIN: string;
		PWD: string;
		NIX_PROFILES: string;
		GSETTINGS_SCHEMA_DIR: string;
		LOGNAME: string;
		CONDA_PREFIX: string;
		___X_CMD_LOG_C_INFO: string;
		npm_config_init_module: string;
		SYSTEMD_EXEC_PID: string;
		GSETTINGS_SCHEMA_DIR_CONDA_BACKUP: string;
		PIXI_PROJECT_MANIFEST: string;
		PIXI_PROJECT_NAME: string;
		_: string;
		NoDefaultCurrentDirectoryInExePath: string;
		FZF_DEFAULT_COMMAND: string;
		CLAUDECODE: string;
		GPU_LLM_DIR: string;
		HOME: string;
		SSH_ASKPASS: string;
		LANG: string;
		LS_COLORS: string;
		npm_package_version: string;
		MEMORY_PRESSURE_WATCH: string;
		STARSHIP_SHELL: string;
		STARSHIP_CONFIG: string;
		NIX_SSL_CERT_FILE: string;
		PIXI_ENVIRONMENT_NAME: string;
		PERL5LIB: string;
		_ble_util_fd_zero: string;
		INVOCATION_ID: string;
		___X_CMD_LOG_C_DEBUG: string;
		MANAGERPID: string;
		PUPPETEER_CACHE_DIR: string;
		PIXI_IN_SHELL: string;
		BASH_SILENCE_DEPRECATION_WARNING: string;
		INIT_CWD: string;
		STARSHIP_SESSION_KEY: string;
		CORRECT_IGNORE_FILE: string;
		npm_lifecycle_script: string;
		CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: string;
		___X_CMD_LOG_C_TF: string;
		npm_config_npm_version: string;
		_ble_util_fd_stdin: string;
		UV_INDEX_STRATEGY: string;
		PIXI_EXE: string;
		TERM: string;
		npm_package_name: string;
		LESS_TERMCAP_mb: string;
		LESS_TERMCAP_me: string;
		LESS_TERMCAP_md: string;
		PERL_MB_OPT: string;
		npm_config_prefix: string;
		CORRECT_IGNORE: string;
		CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS: string;
		USER: string;
		TMUX_PANE: string;
		___X_CMD_LOG_C_ERROR: string;
		CONDA_SHLVL: string;
		PERL_MM_OPT: string;
		DISPLAY: string;
		npm_lifecycle_event: string;
		SHLVL: string;
		LESS_TERMCAP_ue: string;
		_ble_util_fd_stderr: string;
		LESS_TERMCAP_us: string;
		GIT_EDITOR: string;
		PAGER: string;
		PIXI_PROJECT_VERSION: string;
		GOCACHE: string;
		ATUIN_SESSION: string;
		MANAGERPIDFDID: string;
		npm_config_user_agent: string;
		OTEL_EXPORTER_OTLP_METRICS_TEMPORALITY_PREFERENCE: string;
		_ble_util_fd_null: string;
		DEEPSEEK_API_KEY: string;
		userid: string;
		npm_execpath: string;
		PIXI_PROJECT_ROOT: string;
		ATUIN_HISTORY_ID: string;
		LC_CTYPE: string;
		XDG_RUNTIME_DIR: string;
		CONDA_DEFAULT_ENV: string;
		PIXI_ENVIRONMENT_PLATFORMS: string;
		CLAUDE_CODE_ENTRYPOINT: string;
		DEBUGINFOD_URLS: string;
		npm_package_json: string;
		LC_TIME: string;
		DOCKER_HOST: string;
		LC_ALL: string;
		___X_CMD_LOG_C_WARN: string;
		JOURNAL_STREAM: string;
		XDG_DATA_DIRS: string;
		PERL_LOCAL_LIB_ROOT: string;
		shellHome: string;
		npm_config_noproxy: string;
		PATH: string;
		npm_config_node_gyp: string;
		groupid: string;
		DBUS_SESSION_BUS_ADDRESS: string;
		FZF_DEFAULT_OPTS: string;
		npm_config_global_prefix: string;
		HG: string;
		npm_node_execpath: string;
		PUPPETEER_SKIP_CHROMIUM_DOWNLOAD: string;
		OLDPWD: string;
		GOPATH: string;
		TERM_PROGRAM: string;
		_ble_util_fd_stdout: string;
		NODE_ENV: string;
		[key: `PUBLIC_${string}`]: undefined;
		[key: `${string}`]: string | undefined;
	}
}

/**
 * This module provides access to environment variables set _dynamically_ at runtime and that are _publicly_ accessible.
 * 
 * |         | Runtime                                                                    | Build time                                                               |
 * | ------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
 * | Private | [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private) | [`$env/static/private`](https://svelte.dev/docs/kit/$env-static-private) |
 * | Public  | [`$env/dynamic/public`](https://svelte.dev/docs/kit/$env-dynamic-public)   | [`$env/static/public`](https://svelte.dev/docs/kit/$env-static-public)   |
 * 
 * Dynamic environment variables are defined by the platform you're running on. For example if you're using [`adapter-node`](https://github.com/sveltejs/kit/tree/main/packages/adapter-node) (or running [`vite preview`](https://svelte.dev/docs/kit/cli)), this is equivalent to `process.env`.
 * 
 * **_Public_ access:**
 * 
 * - This module _can_ be imported into client-side code
 * - **Only** variables that begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) (which defaults to `PUBLIC_`) are included
 * 
 * > [!NOTE] In `dev`, `$env/dynamic` includes environment variables from `.env`. In `prod`, this behavior will depend on your adapter.
 * 
 * > [!NOTE] To get correct types, environment variables referenced in your code should be declared (for example in an `.env` file), even if they don't have a value until the app is deployed:
 * >
 * > ```env
 * > MY_FEATURE_FLAG=
 * > ```
 * >
 * > You can override `.env` values from the command line like so:
 * >
 * > ```sh
 * > MY_FEATURE_FLAG="enabled" npm run dev
 * > ```
 * 
 * For example, given the following runtime environment:
 * 
 * ```env
 * ENVIRONMENT=production
 * PUBLIC_BASE_URL=http://example.com
 * ```
 * 
 * With the default `publicPrefix` and `privatePrefix`:
 * 
 * ```ts
 * import { env } from '$env/dynamic/public';
 * console.log(env.ENVIRONMENT); // => undefined, not public
 * console.log(env.PUBLIC_BASE_URL); // => "http://example.com"
 * ```
 * 
 * ```
 * 
 * ```
 */
declare module '$env/dynamic/public' {
	export const env: {
		[key: `PUBLIC_${string}`]: string | undefined;
	}
}
