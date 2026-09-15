/** Reactive path-free state for explicit native local-image worker import. */

import {
  importLocalImageWorker,
  localImageWorkerImportApproval,
  type LocalImageAvailabilityMetadata,
  type LocalImageWorkerImportOutcome,
} from "$lib/local-image";

type LocalImageWorkerImportDependencies = {
  importWorker: (approval: ReturnType<typeof localImageWorkerImportApproval>) => Promise<LocalImageWorkerImportOutcome>;
};

const DEFAULT_DEPENDENCIES: LocalImageWorkerImportDependencies = {
  importWorker: importLocalImageWorker,
};

/** Owns one native picker/import action without retaining its selected filesystem location. */
export class LocalImageWorkerImportState {
  active = $state(false);
  feedback = $state("");

  constructor(private readonly dependencies: LocalImageWorkerImportDependencies = DEFAULT_DEPENDENCIES) {}

  /** Imports an eligible exact worker and publishes only fresh path-free readiness. */
  async start(
    metadata: LocalImageAvailabilityMetadata,
    applyAvailability: (availability: LocalImageAvailabilityMetadata) => void,
    refreshAcquisition: () => Promise<void>,
  ): Promise<void> {
    if (this.active || !["worker_missing", "worker_mismatch"].includes(metadata.availability)) return;
    this.active = true;
    this.feedback = "";
    try {
      const outcome = await this.dependencies.importWorker(localImageWorkerImportApproval(metadata));
      applyAvailability(outcome.availability);
      this.feedback = outcome.imported
        ? "Exact worker installed in Bottie’s private cache."
        : "Worker selection cancelled.";
      if (outcome.imported) await refreshAcquisition();
    } catch (error) {
      this.feedback =
        error === "integrity"
          ? "The selected folder did not match the exact reviewed worker bundle."
          : "The local image worker could not be installed safely.";
    } finally {
      this.active = false;
    }
  }
}
