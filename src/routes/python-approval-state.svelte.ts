/** Reactive owner for the process-local Python approval presentation. */

import type { UnlistenFn } from "@tauri-apps/api/event";

import {
  decidePythonApproval,
  getPythonApproval,
  listenForPythonApproval,
  type PythonApprovalDecision,
  type PythonApprovalStatus,
} from "$lib/python-approval";

type PythonApprovalGateway = {
  get: () => Promise<PythonApprovalStatus | null>;
  decide: (requestId: string, decision: PythonApprovalDecision) => Promise<PythonApprovalStatus>;
  listen: (onApproval: (approval: PythonApprovalStatus | null) => void) => Promise<UnlistenFn>;
};

const nativeGateway: PythonApprovalGateway = {
  get: getPythonApproval,
  decide: decidePythonApproval,
  listen: listenForPythonApproval,
};

/** Keeps exact review state visible while one native decision request is in flight. */
export class PythonApprovalState {
  approval = $state<PythonApprovalStatus | null>(null);
  busy = $state(false);
  error = $state("");

  private previewOnly = false;
  private publicationSequence = 0;
  private stopApprovalUpdates?: UnlistenFn;

  constructor(private readonly gateway: PythonApprovalGateway = nativeGateway) {}

  /** Subscribes before loading native state so startup and generation-time proposals cannot be missed. */
  async initialize(): Promise<void> {
    if (this.previewOnly) return;
    const initialPublicationSequence = this.publicationSequence;
    let listenerFailed = false;
    try {
      this.stopApprovalUpdates = await this.gateway.listen((approval) => {
        this.publicationSequence += 1;
        this.approval = approval;
        this.busy = false;
        this.error = "";
      });
    } catch (error) {
      listenerFailed = true;
      this.error = "Bottie could not monitor Python approval requests.";
      console.warn("Could not listen for Python approval requests", error);
    }
    try {
      const approval = await this.gateway.get();
      if (this.publicationSequence === initialPublicationSequence) this.approval = approval;
      if (!listenerFailed) this.error = "";
    } catch {
      this.error = "Bottie could not load the pending Python review.";
    }
  }

  /** Releases the native approval listener when the page is unmounted. */
  dispose(): void {
    this.stopApprovalUpdates?.();
    this.stopApprovalUpdates = undefined;
  }

  /** Records one explicit decision without resending source, purpose, or provider identity. */
  async decide(decision: PythonApprovalDecision): Promise<void> {
    const approval = this.approval;
    if (!approval || approval.phase !== "pending" || this.busy) return;
    this.busy = true;
    this.error = "";
    try {
      this.approval = this.previewOnly
        ? { ...approval, phase: decision === "approve" ? "approved" : "denied" }
        : await this.gateway.decide(approval.requestId, decision);
    } catch {
      this.error = "Bottie could not record that decision. Review the request and try again.";
    } finally {
      this.busy = false;
    }
  }

  /** Installs an inert development-only review without invoking the native process. */
  preview(approval: PythonApprovalStatus): void {
    this.previewOnly = true;
    this.approval = approval;
    this.error = "";
  }

  /** Hides a terminal acknowledgement after the decision remains retained natively. */
  dismissResolved(): void {
    if (this.approval?.phase !== "pending") this.approval = null;
  }
}
