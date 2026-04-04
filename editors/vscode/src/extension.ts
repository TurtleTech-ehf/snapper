import { execFile, execFileSync } from "child_process";
import { promisify } from "util";
import {
    commands,
    env,
    ExtensionContext,
    LogOutputChannel,
    StatusBarAlignment,
    StatusBarItem,
    Uri,
    window,
    workspace,
} from "vscode";
import {
    CloseAction,
    ErrorAction,
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    Executable,
    State,
} from "vscode-languageclient/node";

const execFileAsync = promisify(execFile);

let client: LanguageClient | undefined;
let statusBarItem: StatusBarItem;

function getSnapperPath(): string {
    return workspace.getConfiguration("snapper").get<string>("path", "snapper");
}

function checkBinary(
    snapperPath: string,
    outputChannel: LogOutputChannel,
): boolean {
    try {
        const version = execFileSync(snapperPath, ["--version"], {
            timeout: 5000,
            encoding: "utf-8",
        }).trim();
        outputChannel.appendLine(`Found ${version} at: ${snapperPath}`);
        return true;
    } catch {
        outputChannel.appendLine(`Binary not found: ${snapperPath}`);
        return false;
    }
}

async function showBinaryNotFound(): Promise<void> {
    const action = await window.showErrorMessage(
        "snapper binary not found. Install it or set a custom path.",
        "Install snapper",
        "Set Custom Path",
    );
    if (action === "Install snapper") {
        env.openExternal(
            Uri.parse(
                "https://snapper.turtletech.us/docs/tutorials/quickstart.html",
            ),
        );
    } else if (action === "Set Custom Path") {
        commands.executeCommand(
            "workbench.action.openSettings",
            "snapper.path",
        );
    }
}

async function startClient(
    context: ExtensionContext,
    outputChannel: LogOutputChannel,
): Promise<void> {
    const snapperPath = getSnapperPath();

    if (!checkBinary(snapperPath, outputChannel)) {
        statusBarItem.text = "$(warning) snapper";
        statusBarItem.tooltip = "snapper: binary not found";
        showBinaryNotFound();
        return;
    }

    const runExecutable: Executable = {
        command: snapperPath,
        args: ["lsp"],
    };

    const serverOptions: ServerOptions = {
        run: runExecutable,
        debug: runExecutable,
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: "file", language: "org" },
            { scheme: "file", language: "latex" },
            { scheme: "file", language: "markdown" },
            { scheme: "file", language: "plaintext" },
            { scheme: "file", language: "restructuredtext" },
        ],
        synchronize: {
            fileEvents: workspace.createFileSystemWatcher("**/.snapperrc.toml"),
        },
        outputChannel,
        initializationFailedHandler: (error) => {
            outputChannel.appendLine(`LSP initialization failed: ${error}`);
            window.showErrorMessage(
                "snapper LSP failed to start. Check Output > snapper for details.",
            );
            return false;
        },
        errorHandler: {
            error(_error, _message, count) {
                outputChannel.appendLine(`LSP error: ${_error.message}`);
                if (count && count >= 3) {
                    return { action: ErrorAction.Shutdown };
                }
                return { action: ErrorAction.Continue };
            },
            closed() {
                outputChannel.appendLine("LSP connection closed");
                statusBarItem.text = "$(warning) snapper";
                statusBarItem.tooltip = "snapper: LSP stopped";
                return { action: CloseAction.DoNotRestart };
            },
        },
        middleware: {
            provideDocumentFormattingEdits: async (
                document,
                options,
                token,
                next,
            ) => {
                return next(document, options, token);
            },
        },
    };
    client = new LanguageClient(
        "snapper",
        "snapper Language Server",
        serverOptions,
        clientOptions,
    );

    client.onDidChangeState(({ newState }) => {
        switch (newState) {
            case State.Starting:
                statusBarItem.text = "$(sync~spin) snapper";
                statusBarItem.tooltip = "snapper: starting...";
                break;
            case State.Running:
                statusBarItem.text = "$(check) snapper";
                statusBarItem.tooltip = "snapper: running";
                break;
            case State.Stopped:
                statusBarItem.text = "$(warning) snapper";
                statusBarItem.tooltip = "snapper: stopped";
                break;
        }
    });

    await client.start();
}

export function activate(context: ExtensionContext): void {
    const outputChannel = window.createOutputChannel("snapper", { log: true });
    context.subscriptions.push(outputChannel);

    // Status bar
    statusBarItem = window.createStatusBarItem(StatusBarAlignment.Right, 100);
    statusBarItem.text = "$(loading~spin) snapper";
    statusBarItem.tooltip = "snapper: initializing...";
    statusBarItem.command = "snapper.showOutputChannel";
    statusBarItem.show();
    context.subscriptions.push(statusBarItem);

    // Commands
    context.subscriptions.push(
        commands.registerCommand("snapper.showOutputChannel", () => {
            outputChannel.show();
        }),

        commands.registerCommand("snapper.restartServer", async () => {
            if (client) {
                await client.stop();
                client = undefined;
            }
            await startClient(context, outputChannel);
        }),

        commands.registerCommand("snapper.formatDocument", async () => {
            // Wait for the LSP client to be ready before attempting to format
            if (client && client.state === State.Running) {
                await commands.executeCommand("editor.action.formatDocument");
            } else {
                window.showErrorMessage(
                    "snapper LSP server is not running. Please wait for initialization or check the output panel.",
                );
            }
        }),

        commands.registerCommand("snapper.initProject", async () => {
            const folder = workspace.workspaceFolders?.[0];
            if (!folder) {
                window.showWarningMessage(
                    "Open a folder first to initialize snapper.",
                );
                return;
            }
            const snapperPath = getSnapperPath();
            try {
                const { stdout, stderr } = await execFileAsync(
                    snapperPath,
                    ["init"],
                    {
                        cwd: folder.uri.fsPath,
                        timeout: 10000,
                    },
                );
                if (stdout) {
                    outputChannel.appendLine(stdout);
                }
                if (stderr) {
                    outputChannel.appendLine(stderr);
                }
                window.showInformationMessage(
                    "Created .snapperrc.toml and .gitattributes in workspace root.",
                );
            } catch (err: unknown) {
                const msg = err instanceof Error ? err.message : String(err);
                window.showErrorMessage(`snapper init failed: ${msg}`);
            }
        }),

        commands.registerCommand("snapper.checkFile", async () => {
            const editor = window.activeTextEditor;
            if (!editor) {
                window.showWarningMessage("No active file to check.");
                return;
            }
            const filePath = editor.document.uri.fsPath;
            const snapperPath = getSnapperPath();
            try {
                await execFileAsync(snapperPath, ["--check", filePath], {
                    timeout: 30000,
                });
                window.showInformationMessage("File is already formatted.");
            } catch (err: unknown) {
                // Exit code 1 means file needs formatting
                if (
                    err &&
                    typeof err === "object" &&
                    "code" in err &&
                    err.code === 1
                ) {
                    const action = await window.showWarningMessage(
                        "File needs formatting.",
                        "Format Now",
                    );
                    if (action === "Format Now") {
                        commands.executeCommand("editor.action.formatDocument");
                    }
                } else {
                    const msg =
                        err instanceof Error ? err.message : String(err);
                    window.showErrorMessage(`snapper check failed: ${msg}`);
                }
            }
        }),

        commands.registerCommand("snapper.showDiff", async () => {
            const editor = window.activeTextEditor;
            if (!editor) {
                window.showWarningMessage("No active file.");
                return;
            }
            const filePath = editor.document.uri.fsPath;
            const snapperPath = getSnapperPath();
            try {
                const { stdout } = await execFileAsync(
                    snapperPath,
                    ["--diff", filePath],
                    {
                        timeout: 30000,
                    },
                );
                if (stdout) {
                    outputChannel.appendLine("--- Diff preview ---");
                    outputChannel.appendLine(stdout);
                    outputChannel.show();
                } else {
                    window.showInformationMessage("No changes needed.");
                }
            } catch (err: unknown) {
                // Exit code 1 means there are diffs to show
                if (err && typeof err === "object" && "stdout" in err) {
                    const stdout = (err as { stdout: string }).stdout;
                    if (stdout) {
                        outputChannel.appendLine("--- Diff preview ---");
                        outputChannel.appendLine(stdout);
                        outputChannel.show();
                    }
                } else {
                    const msg =
                        err instanceof Error ? err.message : String(err);
                    window.showErrorMessage(`snapper diff failed: ${msg}`);
                }
            }
        }),
    );

    // Watch for config changes
    context.subscriptions.push(
        workspace.onDidChangeConfiguration((e) => {
            if (e.affectsConfiguration("snapper.path")) {
                commands.executeCommand("snapper.restartServer");
            }
        }),
    );

    // Start the LSP client
    startClient(context, outputChannel);

    // Welcome message (first activation only)
    if (!context.globalState.get("snapper.welcomed")) {
        window
            .showInformationMessage(
                'snapper is active! Use "Format Document" (Shift+Alt+F) to format, or set editor.formatOnSave in settings.',
                "Learn More",
            )
            .then((action) => {
                if (action === "Learn More") {
                    env.openExternal(
                        Uri.parse("https://snapper.turtletech.us/docs/"),
                    );
                }
            });
        context.globalState.update("snapper.welcomed", true);
    }
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
