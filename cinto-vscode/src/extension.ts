import * as vscode from "vscode";
import { SidebarProvider } from "./sidebar/SidebarProvider";

export function activate(context: vscode.ExtensionContext): void {
  const provider = new SidebarProvider(context);

  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider("cinto.sidebar", provider, {
      webviewOptions: { retainContextWhenHidden: true },
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("cinto.runTask", () => {
      provider.focusAndPromptTask();
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("cinto.stopTask", () => {
      provider.stopTask();
    })
  );
}

export function deactivate(): void {
  // SidebarProvider.dispose() handles killing the agent process.
}
