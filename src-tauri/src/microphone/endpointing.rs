//! Deterministic end-of-turn decisions over native voice activity and partial transcript text.
//!
//! The machine answers one question per capture: has the user finished speaking? It owns no threads, performs no I/O,
//! and never reads the clock itself, so every rule is reproducible from the `Instant` values a caller supplies.
//!
//! Ordering contract: **voice activity decides silence, the transcript decides completeness, and the timers decide when
//! to stop waiting.** Every timer is relative to the newest confirmed speech frame and never to the arrival time of a
//! transcript, so a slow recognition pass can never hold a turn open on its own.
//!
//! Phase 0 keeps this module unwired: nothing outside these tests constructs an [`EndPointer`] yet, so the unused-item
//! noise is silenced for non-test builds only, matching the existing `image_generation` precedent. Remove this attribute
//! when the capture loop starts driving it.

#![cfg_attr(not(test), allow(dead_code))]

use std::time::Instant;

use super::vad::VoiceActivity;

/// Silence deadline used while the transcript is still changing, first extension.
const CHANGING_EXTENSION_MS: u64 = 750;
/// Silence deadline used while the transcript is still changing, second and final extension.
const CHANGING_EXTENSION_FINAL_MS: u64 = 1_000;

/// Stable reason one turn ended without the user pressing Stop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EndReason {
    /// The newest two partials were byte-identical, so local recognition has settled.
    StableTranscript,
    /// The newest partial ended in sentence-final punctuation.
    TerminalPunctuation,
    /// The silence deadline expired while the transcript was still changing.
    SilenceTimeout,
    /// Confirmed speech produced no transcript at all within the empty-turn window.
    NoSpeech,
    /// The hard silence cap expired before any other rule could decide.
    HardCap,
}

/// One path-free end-of-turn decision for the current capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EndpointDecision {
    /// Keep listening; no rule has finished the turn yet.
    Continue,
    /// Finish the turn now for exactly one reason.
    End {
        /// Which rule finished the turn.
        reason: EndReason,
    },
}

/// Timing policy for automatic end-of-turn detection.
///
/// The phase-0 defaults follow the endpointing design: a 480 ms silence hangover, changes allowed to extend it to
/// 750 ms and then 1,000 ms, a 1,200 ms hard cap, and 700 ms of silence before an empty turn is reported as
/// [`EndReason::NoSpeech`]. `min_speech_ms` matches the 60 ms speech onset confirmation Bottie's existing energy
/// detector already requires.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EndpointConfig {
    /// Shortest speech run that counts as the user having started a turn.
    pub min_speech_ms: u64,
    /// Silence after the newest speech frame before a settled transcript ends the turn.
    pub silence_hangover_ms: u64,
    /// Absolute silence ceiling that overrides every other rule.
    pub hard_cap_ms: u64,
    /// Silence after speech before a turn with no transcript at all is abandoned.
    pub empty_turn_silence_ms: u64,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            min_speech_ms: 60,
            silence_hangover_ms: 480,
            hard_cap_ms: 1_200,
            empty_turn_silence_ms: 700,
        }
    }
}

/// Deterministic end-of-turn state for one armed capture.
pub(super) struct EndPointer {
    config: EndpointConfig,
    armed: bool,
    speech_run_started_at: Option<Instant>,
    speech_run_confirmed: bool,
    last_speech_at: Option<Instant>,
    silence_deadline_ms: u64,
    transcript: Option<String>,
    transcript_stable: bool,
    saw_transcript: bool,
}

impl EndPointer {
    /// Creates one idle machine under an explicit timing policy.
    pub(super) fn new(config: EndpointConfig) -> Self {
        Self {
            config,
            armed: false,
            speech_run_started_at: None,
            speech_run_confirmed: false,
            last_speech_at: None,
            silence_deadline_ms: config.silence_hangover_ms,
            transcript: None,
            transcript_stable: false,
            saw_transcript: false,
        }
    }

    /// Arms this machine for one new turn, discarding every earlier decision.
    ///
    /// Every timer here is relative to the last confirmed speech frame, so arming records no anchor of its own. The
    /// timestamp is accepted so the frame loop can hand every entry point the same monotonic clock.
    pub(super) fn armed(&mut self, _now: Instant) {
        self.clear_turn();
        self.armed = true;
    }

    /// Applies one frame of voice activity and returns the resulting decision.
    ///
    /// The caller applies every frame in order, so a speech run is one contiguous sequence of [`VoiceActivity::Speech`]
    /// frames closed by a [`VoiceActivity::Silence`] frame. A run shorter than [`EndpointConfig::min_speech_ms`] is
    /// treated as an onset blip: it neither starts nor extends a turn and never restarts any timer, so a microphone click
    /// or a single noisy frame cannot postpone or produce an end-of-turn decision.
    pub(super) fn on_activity(
        &mut self,
        now: Instant,
        activity: VoiceActivity,
    ) -> EndpointDecision {
        if !self.armed {
            return EndpointDecision::Continue;
        }
        match activity {
            VoiceActivity::Speech => self.on_speech_frame(now),
            VoiceActivity::Silence => {
                self.speech_run_started_at = None;
                self.speech_run_confirmed = false;
                self.evaluate(now)
            }
        }
    }

    /// Applies one partial transcript and returns the resulting decision.
    ///
    /// The arrival time is used only to evaluate timers anchored at the newest speech frame. It never restarts a
    /// silence timer, so a late transcript cannot extend a turn past [`EndpointConfig::hard_cap_ms`].
    pub(super) fn on_transcript(&mut self, now: Instant, text: &str) -> EndpointDecision {
        if !self.armed {
            return EndpointDecision::Continue;
        }
        self.transcript_stable = self
            .transcript
            .as_deref()
            .is_some_and(|previous| previous == text);
        self.saw_transcript |= !text.trim().is_empty();
        self.transcript = Some(text.to_owned());
        self.evaluate(now)
    }

    /// Clears every decision and returns this machine to idle.
    pub(super) fn reset(&mut self) {
        self.clear_turn();
        self.armed = false;
    }

    /// Confirms a speech run only once it is long enough to be deliberate speech.
    fn on_speech_frame(&mut self, now: Instant) -> EndpointDecision {
        let started = *self.speech_run_started_at.get_or_insert(now);
        if elapsed_ms(started, now) >= self.config.min_speech_ms {
            self.speech_run_confirmed = true;
        }
        if self.speech_run_confirmed {
            self.last_speech_at = Some(now);
            self.silence_deadline_ms = self.config.silence_hangover_ms;
        }
        EndpointDecision::Continue
    }

    /// Evaluates every timer against the newest confirmed speech frame.
    fn evaluate(&mut self, now: Instant) -> EndpointDecision {
        let Some(last_speech_at) = self.last_speech_at else {
            return EndpointDecision::Continue;
        };
        let silence_ms = elapsed_ms(last_speech_at, now);
        if silence_ms >= self.config.hard_cap_ms {
            return EndpointDecision::End {
                reason: EndReason::HardCap,
            };
        }
        if !self.saw_transcript {
            return if silence_ms >= self.config.empty_turn_silence_ms {
                EndpointDecision::End {
                    reason: EndReason::NoSpeech,
                }
            } else {
                EndpointDecision::Continue
            };
        }
        if let Some(reason) = self.settled_reason() {
            return if silence_ms >= self.config.silence_hangover_ms {
                EndpointDecision::End { reason }
            } else {
                EndpointDecision::Continue
            };
        }
        if silence_ms < self.silence_deadline_ms {
            return EndpointDecision::Continue;
        }
        match self.next_deadline() {
            Some(deadline) => {
                self.silence_deadline_ms = deadline;
                EndpointDecision::Continue
            }
            None => EndpointDecision::End {
                reason: EndReason::SilenceTimeout,
            },
        }
    }

    /// Returns why the newest transcript looks finished, if it does.
    fn settled_reason(&self) -> Option<EndReason> {
        if self.transcript_stable {
            return Some(EndReason::StableTranscript);
        }
        if self
            .transcript
            .as_deref()
            .is_some_and(ends_with_terminal_punctuation)
        {
            return Some(EndReason::TerminalPunctuation);
        }
        None
    }

    /// Returns one bounded extension for a transcript that is still changing.
    fn next_deadline(&self) -> Option<u64> {
        let candidate = if self.silence_deadline_ms < CHANGING_EXTENSION_MS {
            CHANGING_EXTENSION_MS
        } else if self.silence_deadline_ms < CHANGING_EXTENSION_FINAL_MS {
            CHANGING_EXTENSION_FINAL_MS
        } else {
            return None;
        };
        (candidate < self.config.hard_cap_ms).then_some(candidate)
    }

    /// Clears the per-turn state without changing whether this machine is armed.
    fn clear_turn(&mut self) {
        self.speech_run_started_at = None;
        self.speech_run_confirmed = false;
        self.last_speech_at = None;
        self.silence_deadline_ms = self.config.silence_hangover_ms;
        self.transcript = None;
        self.transcript_stable = false;
        self.saw_transcript = false;
    }
}

/// Reports whether trimmed text ends in sentence-final punctuation.
fn ends_with_terminal_punctuation(text: &str) -> bool {
    matches!(text.trim_end().chars().next_back(), Some('.' | '?' | '!'))
}

/// Saturates a monotonic interval into whole milliseconds.
fn elapsed_ms(start: Instant, end: Instant) -> u64 {
    u64::try_from(end.saturating_duration_since(start).as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// Returns a monotonic anchor every test derives its offsets from.
    fn clock() -> Instant {
        Instant::now()
    }

    fn at(base: Instant, milliseconds: u64) -> Instant {
        base + Duration::from_millis(milliseconds)
    }

    fn end(reason: EndReason) -> EndpointDecision {
        EndpointDecision::End { reason }
    }

    fn armed_pointer(base: Instant) -> EndPointer {
        let mut pointer = EndPointer::new(EndpointConfig::default());
        pointer.armed(base);
        pointer
    }

    /// Feeds the four 20 ms frames that confirm a 60 ms speech onset, leaving the newest speech frame at base + 60 ms.
    fn confirm_speech(pointer: &mut EndPointer, start: Instant) {
        for milliseconds in [0, 20, 40, 60] {
            assert_eq!(
                pointer.on_activity(at(start, milliseconds), VoiceActivity::Speech),
                EndpointDecision::Continue
            );
        }
    }

    fn silence(pointer: &mut EndPointer, now: Instant) -> EndpointDecision {
        pointer.on_activity(now, VoiceActivity::Silence)
    }

    #[test]
    fn no_end_before_silence_hangover() {
        let base = clock();
        let mut pointer = armed_pointer(base);
        confirm_speech(&mut pointer, base);
        assert_eq!(
            pointer.on_transcript(at(base, 100), "are you there?"),
            EndpointDecision::Continue
        );
        // 479 ms of silence is never enough, however settled the transcript looks.
        assert_eq!(
            silence(&mut pointer, at(base, 539)),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 540)),
            end(EndReason::TerminalPunctuation)
        );
    }

    #[test]
    fn silence_timer_resets_when_speech_returns() {
        let base = clock();
        let mut pointer = armed_pointer(base);
        confirm_speech(&mut pointer, base);
        assert_eq!(
            pointer.on_transcript(at(base, 100), "are you there?"),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 400)),
            EndpointDecision::Continue
        );
        // Speech returns: the run and the silence timer restart from the newest speech frame.
        for milliseconds in [420, 480] {
            assert_eq!(
                pointer.on_activity(at(base, milliseconds), VoiceActivity::Speech),
                EndpointDecision::Continue
            );
        }
        assert_eq!(
            silence(&mut pointer, at(base, 700)),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 959)),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 960)),
            end(EndReason::TerminalPunctuation)
        );
    }

    #[test]
    fn stable_transcript_ends_at_silence_hangover() {
        let base = clock();
        let mut pointer = armed_pointer(base);
        confirm_speech(&mut pointer, base);
        assert_eq!(
            pointer.on_transcript(at(base, 100), "hello there"),
            EndpointDecision::Continue
        );
        // Two identical partials are stable, but stability alone cannot end a turn early.
        assert_eq!(
            pointer.on_transcript(at(base, 200), "hello there"),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 539)),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 540)),
            end(EndReason::StableTranscript)
        );
    }

    #[test]
    fn terminal_punctuation_ends_at_silence_hangover() {
        let base = clock();
        for text in ["done.", "done?", "done!"] {
            let mut pointer = armed_pointer(base);
            confirm_speech(&mut pointer, base);
            assert_eq!(
                pointer.on_transcript(at(base, 100), text),
                EndpointDecision::Continue
            );
            assert_eq!(
                silence(&mut pointer, at(base, 540)),
                end(EndReason::TerminalPunctuation),
                "{text} should finish the turn"
            );
        }
        // Punctuation that is not sentence-final leaves the turn open.
        let mut pointer = armed_pointer(base);
        confirm_speech(&mut pointer, base);
        assert_eq!(
            pointer.on_transcript(at(base, 100), "done,"),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 540)),
            EndpointDecision::Continue
        );
    }

    #[test]
    fn changing_transcript_extends_deadline_then_times_out() {
        let base = clock();
        let mut pointer = armed_pointer(base);
        confirm_speech(&mut pointer, base);
        assert_eq!(
            pointer.on_transcript(at(base, 100), "one two"),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 540)),
            EndpointDecision::Continue
        );
        assert_eq!(
            pointer.on_transcript(at(base, 600), "one two three"),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 810)),
            EndpointDecision::Continue
        );
        assert_eq!(
            pointer.on_transcript(at(base, 900), "one two three four"),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 1_059)),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 1_060)),
            end(EndReason::SilenceTimeout)
        );
    }

    #[test]
    fn hard_cap_ends_on_a_late_transcript() {
        let base = clock();
        let mut pointer = armed_pointer(base);
        confirm_speech(&mut pointer, base);
        assert_eq!(
            pointer.on_transcript(at(base, 100), "half a thought"),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 540)),
            EndpointDecision::Continue
        );
        // No activity frames arrive; the transcript advances only after the hard silence cap.
        assert_eq!(
            pointer.on_transcript(at(base, 1_259), "half a thought, still"),
            EndpointDecision::Continue
        );
        assert_eq!(
            pointer.on_transcript(at(base, 1_260), "half a thought, still,"),
            end(EndReason::HardCap)
        );
    }

    #[test]
    fn hard_cap_wins_over_a_stable_late_transcript() {
        let base = clock();
        let mut pointer = armed_pointer(base);
        confirm_speech(&mut pointer, base);
        assert_eq!(
            pointer.on_transcript(at(base, 100), "hello"),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 540)),
            EndpointDecision::Continue
        );
        // This partial is byte-identical to the previous one, which would normally finish the turn immediately; the
        // silence cap is checked first, so it cannot rescue a turn that has already aged out.
        assert_eq!(
            pointer.on_transcript(at(base, 1_300), "hello"),
            end(EndReason::HardCap)
        );
    }

    #[test]
    fn empty_turn_ends_with_no_speech() {
        let base = clock();
        let mut pointer = armed_pointer(base);
        confirm_speech(&mut pointer, base);
        assert_eq!(
            silence(&mut pointer, at(base, 759)),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 760)),
            end(EndReason::NoSpeech)
        );
    }

    #[test]
    fn short_speech_blip_does_not_restart_the_empty_turn_timer() {
        let base = clock();
        let mut pointer = armed_pointer(base);
        confirm_speech(&mut pointer, base);
        // A silence frame closes the first speech run, so the next speech frame opens a new one.
        assert_eq!(
            silence(&mut pointer, at(base, 80)),
            EndpointDecision::Continue
        );
        // One 20 ms run is far below the deliberate-speech minimum, so it never moves a timer.
        assert_eq!(
            pointer.on_activity(at(base, 400), VoiceActivity::Speech),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 700)),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 760)),
            end(EndReason::NoSpeech)
        );
    }

    #[test]
    fn blank_transcript_is_not_completed_speech() {
        let base = clock();
        let mut pointer = armed_pointer(base);
        confirm_speech(&mut pointer, base);
        assert_eq!(
            pointer.on_transcript(at(base, 100), "   "),
            EndpointDecision::Continue
        );
        // Repeated blank text is "stable" but is still no transcript at all.
        assert_eq!(
            pointer.on_transcript(at(base, 200), "   "),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 760)),
            end(EndReason::NoSpeech)
        );
    }

    #[test]
    fn unarmed_pointer_ignores_activity_and_transcripts() {
        let base = clock();
        let mut pointer = EndPointer::new(EndpointConfig::default());
        assert_eq!(
            pointer.on_activity(at(base, 0), VoiceActivity::Speech),
            EndpointDecision::Continue
        );
        assert_eq!(
            pointer.on_activity(at(base, 10_000), VoiceActivity::Silence),
            EndpointDecision::Continue
        );
        assert_eq!(
            pointer.on_transcript(at(base, 10_000), "done."),
            EndpointDecision::Continue
        );
        pointer.armed(at(base, 20_000));
        confirm_speech(&mut pointer, at(base, 20_000));
        assert_eq!(
            pointer.on_transcript(at(base, 20_100), "done."),
            EndpointDecision::Continue
        );
        assert_eq!(
            silence(&mut pointer, at(base, 20_540)),
            end(EndReason::TerminalPunctuation)
        );
    }

    #[test]
    fn reset_returns_the_machine_to_idle() {
        let base = clock();
        let mut pointer = armed_pointer(base);
        confirm_speech(&mut pointer, base);
        pointer.reset();
        assert_eq!(
            silence(&mut pointer, at(base, 500)),
            EndpointDecision::Continue
        );
        assert_eq!(
            pointer.on_transcript(at(base, 600), "done."),
            EndpointDecision::Continue
        );
    }
}
