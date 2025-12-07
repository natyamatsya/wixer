import { workspace, ExtensionContext } from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
  // Get wixer executable path from settings, default to 'wixer' (searches PATH)
  const config = workspace.getConfiguration('wixer');
  const executablePath = config.get<string>('executablePath', 'wixer');
  
  const serverOptions: ServerOptions = {
    run: { 
      command: executablePath, 
      args: ['lsp'], 
      transport: TransportKind.stdio 
    },
    debug: { 
      command: executablePath, 
      args: ['lsp'], 
      transport: TransportKind.stdio 
    }
  };

  const clientOptions: LanguageClientOptions = {
    // Register for WiX YAML files
    documentSelector: [
      { scheme: 'file', language: 'wix-yaml' },
      { scheme: 'file', pattern: '**/*.wxs.yaml' },
      { scheme: 'file', pattern: '**/*.wxi.yaml' },
      { scheme: 'file', pattern: '**/*.wxl.yaml' }
    ],
    synchronize: {
      // Watch for wixer.yaml config changes
      fileEvents: workspace.createFileSystemWatcher('**/wixer.yaml')
    }
  };

  client = new LanguageClient(
    'wixer',
    'Wixer Language Server',
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
