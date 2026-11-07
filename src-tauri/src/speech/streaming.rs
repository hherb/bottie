//! Streaming speech: reduce a token stream to speakable chunks and play them in order.

use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Total speakable text one session will accept before failing closed.
const MAX_STREAM_TEXT_BYTES: usize = 32 * 1_024;
/// A first chunk is short so playback starts on the first clause rather than the first sentence.
const FIRST_CHUNK_MAX_WORDS: usize = 4;
/// Later chunks follow sentence/clause boundaries but stay bounded for cancellation granularity.
const NEXT_CHUNK_MAX_WORDS: usize = 20;
/// Poll cadence while waiting for one chunk to finish playing.
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// A synchronous sink that never becomes idle is treated as failed rather than waited on forever.
const IDLE_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
/// A held `[label](destination)` longer than this is spoken as plain text instead of waiting for more deltas.
const MAX_LINK_BYTES: usize = 2 * 1_024;
/// One bounded chunk (at most `NEXT_CHUNK_MAX_WORDS`) that is still playing after this long is treated as stuck.
const CHUNK_PLAYBACK_TIMEOUT: Duration = Duration::from_secs(60);

/// Path-free progress vocabulary for one streamed reply.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionProgress {
    /// No streamed speech has been requested.
    Idle,
    /// Text is accumulating but nothing is currently playing.
    Buffering,
    /// Playback has started; later chunks may still be waiting for text.
    Speaking,
    /// The reply was interrupted; queued chunks were dropped.
    Interrupted,
    /// The session failed and will not speak further text.
    Failed,
}

/// The narrow synchronous speech surface a streaming session needs.
pub(crate) trait SpeechSink: Send + Sync {
    /// Reports whether the underlying engine is not playing; `Err` means the engine failed.
    fn is_idle(&self) -> Result<bool, ()>;
    /// Starts one chunk. Must accept the chunk without waiting for playback to finish.
    fn speak(&self, text: &str) -> Result<(), ()>;
    /// Interrupts current playback and drops anything the engine still holds.
    fn stop(&self) -> Result<(), ()>;
}

enum SessionCommand {
    Delta(String),
    End,
    Interrupt,
    Shutdown,
}

/// Converts provider deltas into speakable text, dropping code fences, link destinations, and inline markup.
#[derive(Default)]
pub(super) struct SpeakableReducer {
    buffer: String,
    accepted_bytes: usize,
    in_code_fence: bool,
    failed: bool,
}

impl SpeakableReducer {
    /// Appends one provider delta and returns whatever text is now safe to speak.
    pub(super) fn push(&mut self, delta: &str) -> String {
        if self.failed {
            return String::new();
        }
        self.accepted_bytes = self.accepted_bytes.saturating_add(delta.len());
        if self.accepted_bytes > MAX_STREAM_TEXT_BYTES {
            self.failed = true;
            self.buffer.clear();
            return String::new();
        }
        self.buffer.push_str(delta);
        self.reduce(false)
    }

    /// Flushes all remaining speakable text at end of stream.
    pub(super) fn finish(&mut self) -> String {
        if self.failed {
            return String::new();
        }
        self.reduce(true)
    }

    fn reduce(&mut self, final_flush: bool) -> String {
        let mut output = String::new();
        loop {
            if self.in_code_fence {
                match self.buffer.find("```") {
                    Some(end) => {
                        self.buffer = self.buffer[end + 3..].to_owned();
                        self.in_code_fence = false;
                    }
                    None => {
                        if final_flush {
                            self.buffer.clear();
                        }
                        break;
                    }
                }
                continue;
            }

            let buffer = self.buffer.clone();
            let fence = buffer.find("```");
            let link = buffer.find('[');
            let boundary = safe_boundary(&buffer);

            let next_fence = match (fence, link) {
                (Some(fence_index), Some(link_index)) => Some(fence_index.min(link_index)),
                (Some(index), None) | (None, Some(index)) => Some(index),
                (None, None) => None,
            };

            match next_fence {
                Some(fence_index) if fence == Some(fence_index) => {
                    let before = &buffer[..fence_index];
                    output.push_str(&strip_markup(before));
                    self.buffer = buffer[fence_index + 3..].to_owned();
                    self.in_code_fence = true;
                }
                Some(link_index) => {
                    if link_index > 0 {
                        output.push_str(&strip_markup(&buffer[..link_index]));
                        self.buffer = buffer[link_index..].to_owned();
                        continue;
                    }
                    // The link starts at the buffer head: speak its label, never its destination.
                    match parse_link(&buffer, final_flush) {
                        Some((label, consumed)) => {
                            output.push_str(&label);
                            self.buffer = buffer[consumed..].to_owned();
                        }
                        None => break,
                    }
                }
                None => match boundary {
                    Some(index) => {
                        output.push_str(&strip_markup(&buffer[..=index]));
                        self.buffer = buffer[index + 1..].to_owned();
                    }
                    None if final_flush => {
                        output.push_str(&strip_markup(&buffer));
                        self.buffer.clear();
                        break;
                    }
                    None => break,
                },
            }
        }
        normalize_spacing(&output)
    }
}

/// Parses a leading `[label](destination)` or bare `[label]` into speakable label text and the bytes it consumed.
///
/// A link never spans a line or a code fence, so a bracket that cannot close before one is plain text and only the
/// `[` itself is consumed. Returns `None` only while the construct may still be completed by a later delta; at end of
/// stream every bracket resolves, so no text after it is ever lost.
fn parse_link(buffer: &str, final_flush: bool) -> Option<(String, usize)> {
    let scope_end = [buffer.find('\n'), buffer.find("```")]
        .into_iter()
        .flatten()
        .min();
    let scope = &buffer[..scope_end.unwrap_or(buffer.len())];
    // Holding stops once the construct can no longer grow: the line ended, the stream ended, or it is too long.
    let settled = final_flush || scope_end.is_some() || scope.len() > MAX_LINK_BYTES;
    let Some(close) = scope.find(']') else {
        return settled.then(|| (String::new(), 1));
    };
    let label = strip_markup(&scope[1..close]);
    let rest = &scope[close + 1..];
    if rest.starts_with('(') {
        return match rest.find(')') {
            Some(end) => Some((label, close + 1 + end + 1)),
            // The destination never closed: speak the label and drop the partial destination.
            None => settled.then_some((label, scope.len())),
        };
    }
    if rest.is_empty() && !settled {
        // A `(destination)` may still arrive in the next delta.
        return None;
    }
    // A bare `[label]`, such as a citation, is spoken as its label.
    Some((label, close + 1))
}

/// Chunks speakable text into bounded playback units.
#[derive(Default)]
pub(super) struct Chunker {
    partial: String,
    emitted_first_chunk: bool,
}

impl Chunker {
    /// Adds speakable text and drains whatever complete chunks it produced into `chunks`.
    pub(super) fn push(&mut self, text: &str, chunks: &mut VecDeque<String>) {
        self.partial.push_str(text);
        loop {
            let Some(chunk) = self.next_chunk() else {
                break;
            };
            chunks.push_back(chunk);
            self.emitted_first_chunk = true;
        }
    }

    /// Flushes the trailing partial chunk at end of stream.
    pub(super) fn finish(&mut self, chunks: &mut VecDeque<String>) {
        let remainder = self.partial.trim();
        if !remainder.is_empty() {
            chunks.push_back(remainder.to_owned());
        }
        self.partial.clear();
    }

    fn next_chunk(&mut self) -> Option<String> {
        let limit = if self.emitted_first_chunk {
            NEXT_CHUNK_MAX_WORDS
        } else {
            FIRST_CHUNK_MAX_WORDS
        };
        let words = self.partial.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            return None;
        }
        let clause = words
            .iter()
            .enumerate()
            .find(|(index, word)| *index < limit && ends_clause(word))
            .map(|(index, _)| index + 1);
        let take = match clause {
            Some(count) => count,
            None if words.len() > limit => limit,
            None => return None,
        };
        let chunk = words[..take].join(" ");
        self.partial = words[take..].join(" ");
        Some(chunk)
    }
}

/// Owns one background worker that plays chunks strictly in order.
pub(crate) struct StreamingSpeechSession {
    commands: mpsc::Sender<SessionCommand>,
    progress: Arc<Mutex<SessionProgress>>,
    /// Checked by the worker on every iteration, so an interrupt never waits behind queued deltas.
    interrupted: Arc<AtomicBool>,
    /// Bounds the unbounded command queue: text past `MAX_STREAM_TEXT_BYTES` is refused at the sender.
    sent_bytes: AtomicUsize,
    thread: Option<JoinHandle<()>>,
}

impl StreamingSpeechSession {
    /// Starts a worker for one reply. The sink is shared with the controller that observes playback.
    pub(crate) fn start(sink: Arc<dyn SpeechSink>) -> Result<Self, ()> {
        let (commands, receiver) = mpsc::channel();
        let progress = Arc::new(Mutex::new(SessionProgress::Buffering));
        let interrupted = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            receiver,
            sink,
            progress: progress.clone(),
            interrupted: interrupted.clone(),
        };
        let thread = thread::Builder::new()
            .name("bottie-streaming-speech".to_owned())
            .spawn(move || worker.run())
            .map_err(|_| ())?;
        Ok(Self {
            commands,
            progress,
            interrupted,
            sent_bytes: AtomicUsize::new(0),
            thread: Some(thread),
        })
    }

    /// Non-blocking push from the generation thread. Every word is kept; only text past the session cap is refused.
    /// The session then finishes speaking what it already queued and reports `Failed`, never a silent truncation.
    pub(crate) fn push_delta(&self, delta: &str) -> Result<(), ()> {
        let previous = self.sent_bytes.fetch_add(delta.len(), Ordering::SeqCst);
        if previous > MAX_STREAM_TEXT_BYTES {
            return Err(());
        }
        self.commands
            .send(SessionCommand::Delta(delta.to_owned()))
            .map_err(|_| ())
    }

    /// Marks the end of the reply; the worker flushes remaining text and drains playback on its own.
    pub(crate) fn end(&self) -> Result<(), ()> {
        self.commands.send(SessionCommand::End).map_err(|_| ())
    }

    /// Interrupts playback; queued chunks are dropped and the engine is stopped.
    pub(crate) fn interrupt(&self) -> Result<(), ()> {
        self.interrupted.store(true, Ordering::SeqCst);
        // Wakes a worker waiting for text; the flag alone already stops it within one poll interval.
        self.commands
            .send(SessionCommand::Interrupt)
            .map_err(|_| ())
    }

    /// Reads only bounded actor state, never waiting for playback.
    pub(crate) fn progress(&self) -> SessionProgress {
        *lock(&self.progress)
    }
}

impl Drop for StreamingSpeechSession {
    fn drop(&mut self) {
        if self.thread.is_none() {
            return;
        }
        // The flag stops the worker on its next iteration even if it never reads the queue again, so the join is
        // bounded by one in-flight engine call.
        self.interrupted.store(true, Ordering::SeqCst);
        let _ = self.commands.send(SessionCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Worker {
    receiver: mpsc::Receiver<SessionCommand>,
    sink: Arc<dyn SpeechSink>,
    progress: Arc<Mutex<SessionProgress>>,
    interrupted: Arc<AtomicBool>,
}

impl Worker {
    fn run(self) {
        let Self {
            receiver,
            sink,
            progress,
            interrupted,
        } = self;
        let mut reducer = SpeakableReducer::default();
        let mut chunker = Chunker::default();
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut session_progress = SessionProgress::Buffering;
        let mut speaking_since: Option<Instant> = None;
        let mut ending = false;
        // The reply passed the text cap: what is already queued still plays, then the session reports failure.
        let mut overflowed = false;
        let mut idle_deadline: Option<Instant> = None;
        let fail = |sink: &Arc<dyn SpeechSink>| {
            let _ = sink.stop();
            *lock(&progress) = SessionProgress::Failed;
        };

        loop {
            if interrupted.load(Ordering::SeqCst) {
                queue.clear();
                let _ = sink.stop();
                *lock(&progress) = SessionProgress::Interrupted;
                return;
            }
            // A chunk ready to play must not wait out the poll interval behind an empty command queue.
            let wait = if speaking_since.is_none() && !queue.is_empty() {
                Duration::ZERO
            } else {
                IDLE_POLL_INTERVAL
            };
            match receiver.recv_timeout(wait) {
                Ok(SessionCommand::Delta(delta)) => {
                    let speakable = reducer.push(&delta);
                    if reducer.failed && !overflowed {
                        overflowed = true;
                        ending = true;
                        chunker.finish(&mut queue);
                    }
                    if !speakable.is_empty() {
                        chunker.push(&speakable, &mut queue);
                    }
                }
                Ok(SessionCommand::End) => {
                    let speakable = reducer.finish();
                    if !speakable.is_empty() {
                        chunker.push(&speakable, &mut queue);
                    }
                    chunker.finish(&mut queue);
                    ending = true;
                }
                // The flag is the interrupt itself; the command only wakes this loop.
                Ok(SessionCommand::Interrupt) => continue,
                Ok(SessionCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = sink.stop();
                    return;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }

            // Advance at most one playback step per iteration so commands stay responsive.
            if let Some(started) = speaking_since {
                match sink.is_idle() {
                    Ok(true) => {
                        speaking_since = None;
                        idle_deadline = None;
                    }
                    Ok(false) if started.elapsed() < CHUNK_PLAYBACK_TIMEOUT => {}
                    Ok(false) | Err(()) => return fail(&sink),
                }
            }

            if speaking_since.is_none() && !queue.is_empty() {
                // An engine error left over from an earlier utterance is not playback; `speak` clears it or fails.
                match sink.is_idle() {
                    Ok(true) | Err(()) => {
                        let chunk = queue.pop_front().expect("a non-empty queue has a front");
                        if sink.speak(&chunk).is_err() {
                            return fail(&sink);
                        }
                        speaking_since = Some(Instant::now());
                        session_progress = SessionProgress::Speaking;
                    }
                    Ok(false) => {
                        let deadline = idle_deadline.get_or_insert_with(Instant::now);
                        if deadline.elapsed() >= IDLE_WAIT_TIMEOUT {
                            return fail(&sink);
                        }
                    }
                }
            }

            if ending && speaking_since.is_none() && queue.is_empty() {
                *lock(&progress) = if overflowed {
                    SessionProgress::Failed
                } else {
                    SessionProgress::Idle
                };
                return;
            }
            *lock(&progress) = session_progress;
        }
    }
}

/// Returns the byte index of the last speakable boundary, if any.
fn safe_boundary(buffer: &str) -> Option<usize> {
    let mut boundary = None;
    for (index, character) in buffer.char_indices() {
        if matches!(character, '.' | '!' | '?' | ';' | ':' | '\n') {
            let following_space = buffer[index + character.len_utf8()..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace);
            if following_space || index + character.len_utf8() == buffer.len() {
                boundary = Some(index);
            }
        }
    }
    boundary
}

fn ends_clause(word: &str) -> bool {
    word.chars()
        .last()
        .is_some_and(|character| matches!(character, '.' | '!' | '?' | ';' | ':' | ','))
}

fn strip_markup(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '*' | '_' | '`' | '#' | '>' | '|' | '[' | ']' | '(' | ')' => {}
            '\n' | '\r' | '\t' => output.push(' '),
            other => output.push(other),
        }
    }
    output
}

fn normalize_spacing(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[derive(Default)]
    struct FakeSink {
        spoken: Mutex<Vec<String>>,
        speaking: AtomicBool,
        stops: AtomicUsize,
        always_speaking: bool,
        fail_speak: bool,
        /// Simulates an engine whose `speak` blocks, such as a first call that loads the model.
        speak_delay: Duration,
        /// Simulates an engine that fails while the chunk it accepted is playing.
        fail_while_playing: bool,
    }

    impl FakeSink {
        fn always_speaking() -> Self {
            Self {
                always_speaking: true,
                ..Self::default()
            }
        }
    }

    impl SpeechSink for FakeSink {
        fn is_idle(&self) -> Result<bool, ()> {
            if self.fail_while_playing && !self.spoken.lock().unwrap().is_empty() {
                return Err(());
            }
            if self.always_speaking {
                Ok(false)
            } else {
                Ok(!self.speaking.load(Ordering::SeqCst))
            }
        }

        fn speak(&self, text: &str) -> Result<(), ()> {
            if self.fail_speak {
                return Err(());
            }
            thread::sleep(self.speak_delay);
            self.spoken.lock().unwrap().push(text.to_owned());
            Ok(())
        }

        fn stop(&self) -> Result<(), ()> {
            self.stops.fetch_add(1, Ordering::SeqCst);
            self.speaking.store(false, Ordering::SeqCst);
            Ok(())
        }
    }

    fn wait_for(condition: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if condition() {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("condition not reached before deadline");
    }

    #[test]
    fn reducer_speaks_link_labels_and_never_destinations() {
        let mut reducer = SpeakableReducer::default();
        let spoken = reducer.push("Read [the docs](https://example.com/secret) now.");
        assert!(spoken.contains("the docs"));
        assert!(!spoken.contains("example.com"));
        assert!(!spoken.contains("https"));
    }

    #[test]
    fn reducer_speaks_text_before_an_unterminated_link_and_holds_the_link() {
        let mut reducer = SpeakableReducer::default();
        assert_eq!(reducer.push("Read [the docs](https://example.com"), "Read");
        let spoken = reducer.push("/secret) now.");
        assert!(spoken.contains("docs"));
        assert!(!spoken.contains("example.com"));
    }

    #[test]
    fn reducer_speaks_a_bare_bracket_and_everything_after_it() {
        let mut reducer = SpeakableReducer::default();
        let mut spoken = reducer.push("See note [1] for details. Then arr[0] is set.");
        spoken.push(' ');
        spoken.push_str(&reducer.finish());
        assert!(spoken.contains("See note 1 for details."), "{spoken}");
        assert!(spoken.contains("is set."), "{spoken}");
    }

    #[test]
    fn reducer_holds_a_closed_label_until_it_knows_whether_a_destination_follows() {
        let mut reducer = SpeakableReducer::default();
        assert_eq!(reducer.push("Read [the docs]"), "Read");
        let spoken = reducer.push("(https://example.com) now.");
        assert!(spoken.contains("the docs now."), "{spoken}");
        assert!(!spoken.contains("example.com"));
    }

    #[test]
    fn reducer_flushes_an_unclosed_bracket_at_end_of_stream() {
        let mut reducer = SpeakableReducer::default();
        assert_eq!(reducer.push("The range [0, 10"), "The range");
        assert_eq!(reducer.finish(), "0, 10");
    }

    #[test]
    fn an_unmatched_bracket_never_reaches_across_a_code_fence() {
        let mut reducer = SpeakableReducer::default();
        let mut spoken = reducer.push(
            "Ranges are half-open [0, n) in Python:\n```python\nx = a[0]\n```\nThat is all. ",
        );
        spoken.push(' ');
        spoken.push_str(&reducer.finish());
        assert!(
            spoken.contains("Ranges are half-open 0, n in Python:"),
            "{spoken}"
        );
        assert!(spoken.contains("That is all."), "{spoken}");
        assert!(!spoken.contains("x = a"), "{spoken}");
    }

    #[test]
    fn an_unmatched_bracket_releases_its_line_without_waiting_for_the_stream_to_end() {
        let mut reducer = SpeakableReducer::default();
        assert_eq!(reducer.push("See [the list"), "See");
        let spoken = reducer.push(" below.\nNext line.");
        assert!(spoken.contains("the list below."), "{spoken}");
    }

    #[test]
    fn reducer_drops_code_fences_and_strips_markup() {
        let mut reducer = SpeakableReducer::default();
        let spoken = reducer.push("Here **is** `code`.\n```fn main() {}\n```");
        assert!(spoken.contains("Here is code."));
        assert!(!spoken.contains("fn main"));
        // The unclosed tail after the visible text must not leak code content.
        assert!(!reducer.finish().contains("fn main"));
    }

    #[test]
    fn chunker_emits_a_short_first_chunk_then_bounded_chunks() {
        let mut chunker = Chunker::default();
        let mut chunks = VecDeque::new();
        chunker.push(
            "Sure, here is the quick answer and then a much longer second sentence follows.",
            &mut chunks,
        );
        assert_eq!(chunks.front().unwrap(), "Sure,");
        chunker.finish(&mut chunks);
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().all(|chunk| !chunk.starts_with(' ')));
    }

    #[test]
    fn chunker_preserves_every_word_in_order() {
        let input =
            "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron";
        let mut chunker = Chunker::default();
        let mut chunks = VecDeque::new();
        chunker.push(input, &mut chunks);
        chunker.finish(&mut chunks);
        let joined = chunks.iter().cloned().collect::<Vec<_>>().join(" ");
        assert_eq!(joined, input);
    }

    #[test]
    fn session_plays_chunks_in_order_then_drains() {
        let sink = Arc::new(FakeSink::default());
        let session = StreamingSpeechSession::start(sink.clone()).unwrap();
        session
            .push_delta("Sure, here is the quick answer. ")
            .unwrap();
        session
            .push_delta("The second sentence is a little longer. ")
            .unwrap();
        session.end().unwrap();
        wait_for(|| session.progress() == SessionProgress::Idle);
        let spoken = sink.spoken.lock().unwrap().clone();
        assert_eq!(spoken.first().map(String::as_str), Some("Sure,"));
        assert!(spoken.len() >= 2);
    }

    #[test]
    fn interrupt_stops_the_sink_and_drops_queued_chunks() {
        let sink = Arc::new(FakeSink::always_speaking());
        let session = StreamingSpeechSession::start(sink.clone()).unwrap();
        session
            .push_delta("This will be queued but never spoken. ")
            .unwrap();
        session.interrupt().unwrap();
        wait_for(|| session.progress() == SessionProgress::Interrupted);
        assert!(sink.stops.load(Ordering::SeqCst) >= 1);
        assert!(sink.spoken.lock().unwrap().is_empty());
    }

    #[test]
    fn a_burst_of_deltas_behind_a_slow_engine_speaks_every_word() {
        let sink = Arc::new(FakeSink {
            speak_delay: Duration::from_millis(30),
            ..FakeSink::default()
        });
        let session = StreamingSpeechSession::start(sink.clone()).unwrap();
        let words = (0..60).map(|index| format!("w{index}")).collect::<Vec<_>>();
        for word in &words {
            session.push_delta(&format!("{word} ")).unwrap();
        }
        session.end().unwrap();
        wait_for(|| session.progress() == SessionProgress::Idle);
        assert_eq!(sink.spoken.lock().unwrap().join(" "), words.join(" "));
    }

    #[test]
    fn interrupt_and_drop_are_prompt_behind_a_long_queue() {
        let sink = Arc::new(FakeSink::always_speaking());
        let session = StreamingSpeechSession::start(sink.clone()).unwrap();
        for _ in 0..200 {
            session
                .push_delta("queued words that are never spoken. ")
                .unwrap();
        }
        let started = Instant::now();
        session.interrupt().unwrap();
        wait_for(|| session.progress() == SessionProgress::Interrupted);
        drop(session);
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(sink.spoken.lock().unwrap().is_empty());
    }

    #[test]
    fn dropping_a_session_that_is_still_speaking_joins_promptly() {
        let sink = Arc::new(FakeSink::always_speaking());
        let session = StreamingSpeechSession::start(sink.clone()).unwrap();
        session.push_delta("One two three four. ").unwrap();
        let started = Instant::now();
        drop(session);
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(sink.stops.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn an_engine_failure_while_a_chunk_plays_fails_the_session() {
        let sink = Arc::new(FakeSink {
            fail_while_playing: true,
            ..FakeSink::default()
        });
        let session = StreamingSpeechSession::start(sink).unwrap();
        session.push_delta("One two three four. ").unwrap();
        wait_for(|| session.progress() == SessionProgress::Failed);
    }

    #[test]
    fn a_reply_over_the_text_cap_finishes_what_it_queued_then_fails() {
        let sink = Arc::new(FakeSink::default());
        let session = StreamingSpeechSession::start(sink.clone()).unwrap();
        // Unspoken code still counts toward the cap, so a large listing crosses it right after the first sentence.
        session
            .push_delta("Here is the whole listing you asked for. ```\n")
            .unwrap();
        let code = "let value = 1;\n".repeat(256);
        let mut refused = false;
        for _ in 0..(MAX_STREAM_TEXT_BYTES / code.len() + 2) {
            refused |= session.push_delta(&code).is_err();
        }
        assert!(refused);
        wait_for(|| session.progress() == SessionProgress::Failed);
        // The sentence queued before the cap was still spoken in full, and playback was not cut off.
        assert_eq!(
            sink.spoken.lock().unwrap().join(" "),
            "Here is the whole listing you asked for."
        );
        assert_eq!(sink.stops.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn failing_speak_marks_the_session_failed() {
        let sink = Arc::new(FakeSink {
            fail_speak: true,
            ..FakeSink::default()
        });
        let session = StreamingSpeechSession::start(sink).unwrap();
        session.push_delta("One two three four. ").unwrap();
        wait_for(|| session.progress() == SessionProgress::Failed);
    }
}
