/** Reactive controller for the one Rust-owned hands-free voice conversation session. */

import { isTauri } from "@tauri-apps/api/core";

import {
  armVoiceReply,
  endVoiceConversation,
  getVoiceConversationStatus,
  INITIAL_VOICE_CONVERSATION_STATUS,
  interruptVoiceTurn,
  resumeVoiceListening,
  startVoiceConversation,
  voiceCommandErrorCode,
  voiceCommandErrorFeedback,
  type VoiceConversationStatus,
} from "$lib/voice-conversation";

const STATUS_POLL_INTERVAL_MS = 200;

/**
 * Owns conversation commands, bounded error feedback, and path-free status polling.
 *
 * The optional observer runs after every applied status, including an unchanged poll, so the page notices a
 * capture that became ready without the native conversation phase having moved. The store itself knows nothing
 * about generation, prompts, or submission.
 */
export class VoiceConversationState {
  status = $state<VoiceConversationStatus>({ ...INITIAL_VOICE_CONVERSATION_STATUS });
  error = $state<string | null>(null);
  starting = $state(false);
  busy = $state(false);
  readonly available = isTauri();

  private readonly onStatus: (status: VoiceConversationStatus) => void;
  private pollTimer?: ReturnType<typeof setInterval>;
  private pollInFlight = false;
  /** Bumped by every action and every polling start; a poll result from an older epoch is discarded. */
  private pollEpoch = 0;
  private actionInFlight = false;

  /** Creates the controller with one optional status observer for turn orchestration. */
  constructor(onStatus: (status: VoiceConversationStatus) => void = () => {}) {
    this.onStatus = onStatus;
  }

  /** Starts the hands-free session after the composer's explicit user action. */
  async start(): Promise<boolean> {
    if (!this.available || this.actionInFlight) return false;
    this.error = null;
    this.starting = true;
    this.actionInFlight = true;
    this.pollEpoch += 1;
    try {
      this.apply(await startVoiceConversation());
      return this.status.active;
    } catch (error) {
      this.fail(error);
      return false;
    } finally {
      this.starting = false;
      this.actionInFlight = false;
    }
  }

  /** Ends the session and stops playback, generation, and conversation capture. */
  async end(): Promise<boolean> {
    if (!this.available || this.actionInFlight) return false;
    this.error = null;
    this.busy = true;
    this.actionInFlight = true;
    this.pollEpoch += 1;
    try {
      this.apply(await endVoiceConversation());
      return !this.status.active;
    } catch (error) {
      this.fail(error);
      return false;
    } finally {
      this.busy = false;
      this.actionInFlight = false;
    }
  }

  /** Stops the current spoken reply and generation, then reopens listening. */
  async interrupt(): Promise<boolean> {
    if (!this.available || this.actionInFlight || !this.status.active) return false;
    this.error = null;
    this.busy = true;
    this.actionInFlight = true;
    this.pollEpoch += 1;
    try {
      this.apply(await interruptVoiceTurn());
      return this.status.active;
    } catch (error) {
      this.fail(error);
      return false;
    } finally {
      this.busy = false;
      this.actionInFlight = false;
    }
  }

  /** Reopens listening so the next turn can start after a reply drained. */
  async resume(): Promise<boolean> {
    if (!this.available || this.actionInFlight || !this.status.active) return false;
    this.error = null;
    this.busy = true;
    this.actionInFlight = true;
    this.pollEpoch += 1;
    try {
      this.apply(await resumeVoiceListening());
      return true;
    } catch (error) {
      this.fail(error);
      return false;
    } finally {
      this.busy = false;
      this.actionInFlight = false;
    }
  }

  /**
   * Marks the reply about to be submitted as spoken by this session.
   *
   * A capture that is still open is recoverable: the caller retries after the next status change, so it is not
   * reported as a failure.
   */
  async armReply(): Promise<boolean> {
    if (!this.available || this.actionInFlight || !this.status.active) return false;
    this.actionInFlight = true;
    this.pollEpoch += 1;
    try {
      this.apply(await armVoiceReply());
      this.error = null;
      return true;
    } catch (error) {
      if (voiceCommandErrorCode(error) !== "capture_active") this.fail(error);
      return false;
    } finally {
      this.actionInFlight = false;
    }
  }

  /** Reads the current phase once without starting native audio or polling. */
  async refresh(): Promise<void> {
    if (!this.available || this.pollInFlight) return;
    this.pollInFlight = true;
    const epoch = this.pollEpoch;
    try {
      const status = await getVoiceConversationStatus();
      // An action or a newer poll since this request began owns the state now.
      if (epoch !== this.pollEpoch) return;
      this.apply(status);
    } catch (error) {
      if (epoch !== this.pollEpoch) return;
      this.fail(error);
      this.stopPolling();
    } finally {
      this.pollInFlight = false;
    }
  }

  /** Stops polling and releases native input when the page is unmounted. */
  dispose(): void {
    this.stopPolling();
    if (this.available && this.status.active) void this.end();
  }

  private apply(status: VoiceConversationStatus): void {
    this.status = status;
    if (status.active) this.startPolling();
    else this.stopPolling();
    this.onStatus(this.status);
  }

  private startPolling(): void {
    this.pollEpoch += 1;
    if (this.pollTimer) return;
    this.pollTimer = setInterval(() => void this.refresh(), STATUS_POLL_INTERVAL_MS);
  }

  private stopPolling(): void {
    if (this.pollTimer) clearInterval(this.pollTimer);
    this.pollTimer = undefined;
  }

  private fail(error: unknown): void {
    this.error = voiceCommandErrorFeedback(voiceCommandErrorCode(error));
    this.stopPolling();
  }
}
