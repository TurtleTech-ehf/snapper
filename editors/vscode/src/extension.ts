import * as path from 'path';
import { workspace, ExtensionContext } from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
    const config = workspace.getConfiguration('snapper');
    const snapperPath = config.get<string>('path', 'snapper');

    const serverOptions: ServerOptions = {
        command: snapperPath,
        args: ['lsp'],
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'org' },
            { scheme: 'file', language: 'latex' },
            { scheme: 'file', language: 'markdown' },
            { scheme: 'file', language: 'plaintext' },
            { scheme: 'file', language: 'restructuredtext' },
        ],
        synchronize: {
            fileEvents: workspace.createFileSystemWatcher('**/.snapperrc.toml'),
        },
    };

    client = new LanguageClient(
        'snapper',
        'snapper Language Server',
        serverOptions,
        clientOptions
    );

    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
