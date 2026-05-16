import * as cp from "child_process";
import { EventEmitter } from "events";

// ---------------------------------------------------------------------------
// Protocol types — mirrors the JSON events defined in docs/vscode-extension-plan.md
// ---------------------------------------------------------------------------

export type KernelEvent =
  | { type: "kernel_ready"; version: string; workspace: string; model: string; context_budget: number }
  | { type: "stage_started"; stage: string }
  | { type: "stage_completed"; stage: string; crp_valid: boolean }
  | { type: "stage_retry"; stage: string; attempt: number; reason: string }
  | { type: "stage_failed"; stage: string; error: string }
  | { type: "stage_skipped"; stage: string; reason: string }
  | { type: "context_pack_ready"; stage: string; chars_used: number; budget: number }
  | { type: "patch_approval_requested"; id: string; path: string; preview: string }
  | { type: "stage_output"; stage: string; search_terms: string[]; relevant_files: string[]; approach: string | null; summary: string | null }
  | { type: "patch_applied"; files_changed: string[] }
  | { type: "workflow_complete"; final_response: string }
  | { type: "workflow_failed"; error: string }
  | { type: "error"; message: string };

// ---------------------------------------------------------------------------
// AgentProcess — manages a single `cinto agent` subprocess
// ---------------------------------------------------------------------------

export declare interface AgentProcess {
  on(event: "kernel_event", listener: (e: KernelEvent) => void): this;
  on(event: "exit", listener: (code: number | null) => void): this;
}

export class AgentProcess extends EventEmitter {
  private proc: cp.ChildProcess | null = null;
  private buffer = "";

  get running(): boolean {
    return this.proc !== null && this.proc.exitCode === null;
  }

  spawn(
    task: string,
    workspace: string,
    binaryPath: string,
    configPath?: string,
    tracesDir?: string
  ): void {
    if (this.running) {
      this.kill();
    }

    const args = ["agent", "--task", task, "--workspace", workspace];
    if (configPath) {
      args.push("--config", configPath);
    }
    if (tracesDir) {
      args.push("--traces-dir", tracesDir);
    }

    this.proc = cp.spawn(binaryPath, args, {
      stdio: ["pipe", "pipe", "pipe"],
    });

    this.buffer = "";

    this.proc.stdout?.on("data", (chunk: Buffer) => {
      this.buffer += chunk.toString("utf8");
      this.flushLines();
    });

    this.proc.stderr?.on("data", (chunk: Buffer) => {
      // Forward stderr as an error event so the UI can surface it.
      const text = chunk.toString("utf8").trim();
      if (text) {
        this.emit("kernel_event", { type: "error", message: text } satisfies KernelEvent);
      }
    });

    this.proc.on("exit", (code) => {
      this.flushLines();
      this.proc = null;
      this.emit("exit", code);
    });

    this.proc.on("error", (err) => {
      this.proc = null;
      this.emit("kernel_event", {
        type: "error",
        message: `Failed to start cinto: ${err.message}`,
      } satisfies KernelEvent);
      this.emit("exit", null);
    });
  }

  /** Send a patch approval response to the kernel via stdin. */
  sendApproval(id: string, approved: boolean): void {
    if (!this.proc?.stdin) return;
    const msg = JSON.stringify({ type: "patch_approval_response", id, approved });
    this.proc.stdin.write(msg + "\n");
  }

  kill(): void {
    if (this.proc) {
      this.proc.kill("SIGTERM");
      this.proc = null;
    }
  }

  private flushLines(): void {
    let idx: number;
    while ((idx = this.buffer.indexOf("\n")) !== -1) {
      const line = this.buffer.slice(0, idx).trim();
      this.buffer = this.buffer.slice(idx + 1);
      if (!line) continue;
      try {
        const event = JSON.parse(line) as KernelEvent;
        this.emit("kernel_event", event);
      } catch {
        // Non-JSON line from the process — surface as an error for debugging.
        this.emit("kernel_event", { type: "error", message: `[stdout] ${line}` } satisfies KernelEvent);
      }
    }
  }
}
