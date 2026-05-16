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
  private currentWorkspace = "";

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
          break;
        case "run":
          this.runTask(msg.task);
          break;
        case "stop":
          this.stopTask();
          break;
        case "approve":
          this.handleApproval(msg.id, msg.approved);
          break;
      }
    });

    webviewView.onDidDispose(() => {
      this.agent.kill();
      this.view = undefined;
    });
  }

  // ---------------------------------------------------------------------------

  focusAndPromptTask(): void {
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

  private handleApproval(id: string, approved: boolean): void {
    this.agent.sendApproval(id, approved);

    // Close any diff editor tab we opened for this review.
    for (const tab of vscode.window.tabGroups.all.flatMap(g => g.tabs)) {
      if (tab.label.startsWith("Review: ")) {
        void vscode.window.tabGroups.close(tab);
        break;
      }
    }
  }

  private runTask(task: string): void {
    if (!task.trim()) return;

    // Remove stale listeners from any previous run.
    this.agent.removeAllListeners();

    const cfg = vscode.workspace.getConfiguration("cinto");
    const binaryPath: string = cfg.get("binaryPath") || "cinto";
    const configPath: string = cfg.get("configPath") || "";
    const tracesDir: string = cfg.get("tracesDir") || "";

    this.currentWorkspace =
      vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? process.cwd();

    this.post({ type: "reset" });

    this.agent.on("kernel_event", (event: KernelEvent) => {
      // Intercept patch approvals to also open the file in the editor.
      if (event.type === "patch_approval_requested") {
        this.openFileForReview(event.path);
      }
      this.post(event);
    });

    this.agent.on("exit", (code: number | null) => {
      this.post({ type: "agent_exited", code });
    });

    this.agent.spawn(
      task,
      this.currentWorkspace,
      binaryPath,
      configPath || undefined,
      tracesDir || undefined
    );
  }

  /** Open the file being patched in the editor so the user has context. */
  private openFileForReview(relativePath: string): void {
    const fullPath = path.join(this.currentWorkspace, relativePath);
    if (!fs.existsSync(fullPath)) return;

    const uri = vscode.Uri.file(fullPath);
    void vscode.window.showTextDocument(uri, {
      viewColumn: vscode.ViewColumn.One,
      preserveFocus: true, // don't steal focus from the sidebar
      preview: true,
    });
  }

  // ---------------------------------------------------------------------------

  private post(message: object): void {
    this.view?.webview.postMessage(message);
  }

  private buildHtml(webview: vscode.Webview): string {
    const mediaDir = vscode.Uri.joinPath(this.context.extensionUri, "media");
    const htmlPath = path.join(mediaDir.fsPath, "panel.html");

    let html = fs.readFileSync(htmlPath, "utf8");

    const nonce = getNonce();
    html = html.replace(/\{\{nonce\}\}/g, nonce);
    html = html.replace(/\{\{cspSource\}\}/g, webview.cspSource);

    return html;
  }
}

function getNonce(): string {
  let text = "";
  const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  for (let i = 0; i < 32; i++) {
    text += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return text;
}
