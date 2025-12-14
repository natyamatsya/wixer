import { workspace, ExtensionContext, window } from 'vscode';
import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import {
  LanguageClient,
  LanguageClientOptions,
  RevealOutputChannelOn,
  ServerOptions,
  Trace
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
  const output = window.createOutputChannel('Wixer');

  const log = (message: string) => {
    output.appendLine(`[${new Date().toISOString()}] ${message}`);
  };

  // Get wixer executable path from settings, default to 'wixer' (searches PATH)
  const config = workspace.getConfiguration('wixer');
  const executablePath = config.get<string>('executablePath', 'wixer');

  log(`[wixer-vscode] activating; executablePath=${executablePath}`);
  log(`[wixer-vscode] node=${process.version} platform=${process.platform} arch=${process.arch}`);

  const traceSetting = config.get<string>('trace.server', 'off');
  const traceLevel = (() => {
    switch (traceSetting) {
      case 'verbose':
        return Trace.Verbose;
      case 'messages':
        return Trace.Messages;
      default:
        return Trace.Off;
    }
  })();
  log(`[wixer-vscode] trace.server=${traceSetting}`);

  // Basic validation for absolute paths to reduce silent failures.
  if (path.isAbsolute(executablePath) && !fs.existsSync(executablePath)) {
    const msg = `Wixer executable not found at configured path: ${executablePath}`;
    log(`[wixer-vscode] ${msg}`);
    window.showErrorMessage(`${msg}. Update VS Code setting wixer.executablePath.`);
  }

  const spawnServer = (): Promise<cp.ChildProcess> => {
    log(`[wixer-vscode] starting language server: ${executablePath} lsp`);
    const child = cp.spawn(executablePath, ['lsp'], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    child.stderr?.on('data', (data: Buffer) => {
      for (const line of data.toString('utf8').split(/\r?\n/)) {
        if (line.trim().length > 0) {
          log(`[wixer lsp] ${line}`);
        }
      }
    });

    child.on('error', (err) => {
      const msg = `Failed to start wixer language server: ${String(err)}`;
      log(`[wixer-vscode] ${msg}`);
      output.show(true);
      window.showErrorMessage(`${msg}. If wixer is installed, set wixer.executablePath to the full path of wixer.exe and reload VS Code.`);
    });

    child.on('exit', (code, signal) => {
      log(`[wixer-vscode] wixer lsp exited (code=${code}, signal=${signal})`);
      if (code !== 0) {
        output.show(true);
        window.showErrorMessage(
          `Wixer language server exited (code=${code}). Check the 'Wixer' Output panel for details.`
        );
      }
    });

    return Promise.resolve(child);
  };

  // Use a custom spawn function so we can capture stderr + lifecycle events.
  const serverOptions: ServerOptions = () => spawnServer();

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
    },
    outputChannel: output,
    traceOutputChannel: output,
    revealOutputChannelOn: RevealOutputChannelOn.Error
  };

  client = new LanguageClient(
    'wixer',
    'Wixer Language Server',
    serverOptions,
    clientOptions
  );

  context.subscriptions.push(output);

  client.onDidChangeState((e) => {
    log(`[wixer-vscode] client state ${e.oldState} -> ${e.newState}`);
  });

  client
    .start()
    .then(async () => {
      log('[wixer-vscode] client.start() resolved');
      await client.setTrace(traceLevel);
      log(`[wixer-vscode] trace enabled via setTrace: ${traceSetting}`);
    })
    .catch((err: unknown) => {
      log(`[wixer-vscode] client.start() failed: ${String(err)}`);
      output.show(true);
      window.showErrorMessage(
        `Failed to start Wixer language client. Check the 'Wixer' Output panel for details.`
      );
    });

  context.subscriptions.push({
    dispose: () => {
      log('[wixer-vscode] disposing extension / stopping client');
      void client.stop();
    }
  });
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
