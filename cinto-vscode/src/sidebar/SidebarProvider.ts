import * as vscode from "vscode";
import * as path from "path";
import * as fs from "fs";
import { AgentProcess, KernelEvent } from "../agent";

// ---------------------------------------------------------------------------
// Messages: webview → extension host
// ---------------------------------------------------------------------------

type WebviewMessage =
  | { command: "run"; task: string }
  | { command: "stop" }
  | { command: "approve"; id: string; approved: boolean }
  | { command: "ready" };

// ---------------------------------------------------------------------------
// SidebarProvider
// ---------------------------------------------------------------------------

export class SidebarProvider implements vscode.WebviewViewProvider, vscode.Disposable {
  private view?: vscode.WebviewView;
  private agent = new AgentProcess();
  private readonly context: vscode.ExtensionContext;

  constructor(context: vscode.ExtensionContext) {
    this.context = context;
  }

  resolveWebviewView(webviewView: vscode.WebviewView): void {
    this.view = webviewView;

    webviewView.webview.options = {
      enableScripts: true,
      localResourceRoots: [
        vscode.Uri.joinPath(this.context.extensionUri, "media"),
      ],
    };

    webviewView.webview.html = this.buildHtml(webviewView.webview);

    webviewView.webview.onDidReceiveMessage((msg: WebviewMessage) => {
      switch (msg.command) {
        case "ready":
          // Webview is loaded — nothing to restore for phase 1.
          break;
        case "run":
          this.runTask(msg.task);
          break;
        case "stop":
          this.stopTask();
          break;
        case "approve":
          this.agent.sendApproval(msg.id, msg.approved);
          break;
      }
    });

    // Clean up when the view is disposed.
    webviewView.onDidDispose(() => {
      this.agent.kill();
      this.view = undefined;
    });
  }

  // ---------------------------------------------------------------------------

  focusAndPromptTask(): void {
    // Reveal the sidebar and tell the webview to focus the task input.
    this.view?.show(true);
    this.post({ type: "focus_input" });
  }

  stopTask(): void {
    this.agent.kill();
    this.post({ type: "agent_stopped" });
  }

  dispose(): void {
    this.agent.kill();
  }

  // ---------------------------------------------------------------------------

  private runTask(task: string): void {
    if (!task.trim()) return;

    const cfg = vscode.workspace.getConfiguration("cinto");
    const binaryPath: string = cfg.get("binaryPath") || "cinto";
    const configPath: string = cfg.get("configPath") || "";
    const tracesDir: string = cfg.get("tracesDir") || "";

    const workspace =
      vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? process.cwd();

    // Reset UI before starting.
    this.post({ type: "reset" });

    this.agent.on("kernel_event", (event: KernelEvent) => {
      this.post(event);
    });

    this.agent.on("exit", (code: number | null) => {
      this.post({ type: "agent_exited", code });
    });

    this.agent.spawn(
      task,
      workspace,
      binaryPath,
      configPath || undefined,
      tracesDir || undefined
    );
  }

  // ---------------------------------------------------------------------------

  private post(message: object): void {
    this.view?.webview.postMessage(message);
  }

  private buildHtml(webview: vscode.Webview): string {
    const mediaDir = vscode.Uri.joinPath(this.context.extensionUri, "media");
    const htmlPath = path.join(mediaDir.fsPath, "panel.html");

    let html = fs.readFileSync(htmlPath, "utf8");

    // Replace the nonce placeholder and CSP.
    const nonce = getNonce();
    html = html.replace(/\{\{nonce\}\}/g, nonce);
    html = html.replace(
      /\{\{cspSource\}\}/g,
      webview.cspSource
    );

    return html;
  }
}

function getNonce(): string {
  let text = "";
  const possible =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  for (let i = 0; i < 32; i++) {
    text += possible.charAt(Math.floor(Math.random() * possible.length));
  }
  return text;
}
