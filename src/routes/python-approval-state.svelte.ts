/** Reactive owner for the process-local Python approval presentation. */

import {
  decidePythonApproval,
  getPythonApproval,
  type PythonApprovalDecision,
  type PythonApprovalStatus,
} from "$lib/python-approval";

type PythonApprovalGateway = {
  get: () => Promise<PythonApprovalStatus | null>;
  decide: (requestId: string, decision: PythonApprovalDecision) => Promise<PythonApprovalStatus>;
};

const nativeGateway: PythonApprovalGateway = {
  get: getPythonApproval,
  decide: decidePythonApproval,
};

/** Keeps exact review state visible while one native decision request is in flight. */
export class PythonApprovalState {
  approval = $state<PythonApprovalStatus | null>(null);
  busy = $state(false);
  error = $state("");

  private previewOnly = false;

  constructor(private readonly gateway: PythonApprovalGateway = nativeGateway) {}

  /** Loads any process-local proposal created before WebView initialization. */
  async initialize(): Promise<void> {
    try {
      this.approval = await this.gateway.get();
      this.error = "";
    } catch {
      this.error = "Bottie could not load the pending Python review.";
    }
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
