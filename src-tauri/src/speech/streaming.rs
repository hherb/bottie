//! Streaming speech: reduce a token stream to speakable chunks and play them in order.

//!
//! Phase 0 keeps this module unwired: nothing outside these tests constructs a session yet, so the unused-item noise is
//! silenced for non-test builds only, matching the existing `image_generation` precedent. Remove this attribute when the
//! generation tap starts driving it.

#![cfg_attr(not(test), allow(dead_code))]

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Total speakable text one session will accept before failing closed.
const MAX_STREAM_TEXT_BYTES: usize = 32 * 1_024;
/// A first chunk is short so playback starts on the first clause rather than the first sentence.
const FIRST_CHUNK_MAX_WORDS: usize = 4;
/// Later chunks follow sentence/clause boundaries but stay bounded for cancellation granularity.
const NEXT_CHUNK_MAX_WORDS: usize = 20;
/// Bounded command queue between the generation thread and the speech worker.
const MAX_QUEUED_COMMANDS: usize = 4;
/// Poll cadence while waiting for one chunk to finish playing.
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// A synchronous sink that never becomes idle is treated as failed rather than waited on forever.
const IDLE_WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Path-free progress vocabulary for one streamed reply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SessionProgress {
    /// No streamed speech has been requested.
    Idle,
    /// Text is accumulating but nothing is currently playing.
    Buffering,
    /// At least one chunk is playing or queued for playback.
    Speaking,
    /// The reply was interrupted; queued chunks were dropped.
    Interrupted,
    /// The session failed and will not speak further text.
    Failed,
}

/// The narrow synchronous speech surface a streaming session needs.
pub(super) trait SpeechSink: Send + Sync {
    /// Reports whether the underlying engine is currently playing.
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

/// Converts provider deltas into speakable text without ever speaking Markdown structure.
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
                    match parse_link(&buffer) {
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

/// Parses a leading `[label](destination)` or `[label]` construct into speakable label text.
fn parse_link(buffer: &str) -> Option<(String, usize)> {
    let close = buffer.find(']')?;
    let label = strip_markup(&buffer[1..close]);
    let mut consumed = close + 1;
    if buffer[consumed..].starts_with('(') {
        let end = buffer[consumed..].find(')')?;
        consumed += end + 1;
    } else {
        // An unterminated destination is still an incomplete link; hold until more text arrives.
        return None;
    }
    Some((label, consumed))
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
pub(super) struct StreamingSpeechSession {
    commands: mpsc::SyncSender<SessionCommand>,
    progress: Arc<Mutex<SessionProgress>>,
    thread: Option<JoinHandle<()>>,
}

impl StreamingSpeechSession {
    /// Starts a worker for one reply. The sink is shared with the controller that observes playback.
    pub(super) fn start(sink: Arc<dyn SpeechSink>) -> Result<Self, ()> {
        let (commands, receiver) = mpsc::sync_channel(MAX_QUEUED_COMMANDS);
        let progress = Arc::new(Mutex::new(SessionProgress::Buffering));
        let shared = progress.clone();
        let thread = thread::Builder::new()
            .name("bottie-streaming-speech".to_owned())
            .spawn(move || run_worker(receiver, sink, shared))
            .map_err(|_| ())?;
        Ok(Self {
            commands,
            progress,
            thread: Some(thread),
        })
    }

    /// Non-blocking push from the generation thread; a full queue fails closed instead of stalling.
    pub(super) fn push_delta(&self, delta: &str) -> Result<(), ()> {
        self.commands
            .try_send(SessionCommand::Delta(delta.to_owned()))
            .map_err(|_| ())
    }

    /// Flushes remaining text and lets the worker drain playback before returning.
    pub(super) fn end(&self) -> Result<(), ()> {
        self.commands.send(SessionCommand::End).map_err(|_| ())
    }

    /// Interrupts playback; queued chunks are dropped and the engine is stopped.
    pub(super) fn interrupt(&self) -> Result<(), ()> {
        self.commands
            .try_send(SessionCommand::Interrupt)
            .map_err(|_| ())
    }

    /// Reads only bounded actor state, never waiting for playback.
    pub(super) fn progress(&self) -> SessionProgress {
        *lock(&self.progress)
    }
}

impl Drop for StreamingSpeechSession {
    fn drop(&mut self) {
        if self.thread.is_none() {
            return;
        }
        // Interrupt first so an in-flight chunk is stopped; then shut the worker down and join.
        let _ = self.commands.try_send(SessionCommand::Interrupt);
        let _ = self.commands.try_send(SessionCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_worker(
    receiver: mpsc::Receiver<SessionCommand>,
    sink: Arc<dyn SpeechSink>,
    progress: Arc<Mutex<SessionProgress>>,
) {
    let mut reducer = SpeakableReducer::default();
    let mut chunker = Chunker::default();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut session_progress = SessionProgress::Buffering;
    let mut speaking = false;
    let mut ending = false;
    let mut idle_deadline: Option<Instant> = None;

    loop {
        match receiver.recv_timeout(IDLE_POLL_INTERVAL) {
            Ok(SessionCommand::Delta(delta)) => {
                let speakable = reducer.push(&delta);
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
            Ok(SessionCommand::Interrupt) => {
                queue.clear();
                let _ = sink.stop();
                *lock(&progress) = SessionProgress::Interrupted;
                return;
            }
            Ok(SessionCommand::Shutdown) => {
                let _ = sink.stop();
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = sink.stop();
                return;
            }
        }

        // Advance at most one playback step per iteration so commands stay responsive.
        if speaking {
            match sink.is_idle() {
                Ok(true) => {
                    speaking = false;
                    idle_deadline = None;
                }
                Ok(false) => {}
                Err(()) => {
                    *lock(&progress) = SessionProgress::Failed;
                    return;
                }
            }
        }

        if !speaking && !queue.is_empty() {
            match sink.is_idle() {
                Ok(true) => {
                    let chunk = queue.pop_front().expect("a non-empty queue has a front");
                    if sink.speak(&chunk).is_err() {
                        *lock(&progress) = SessionProgress::Failed;
                        return;
                    }
                    speaking = true;
                    session_progress = SessionProgress::Speaking;
                }
                Ok(false) => {
                    let deadline = idle_deadline.get_or_insert_with(Instant::now);
                    if deadline.elapsed() >= IDLE_WAIT_TIMEOUT {
                        *lock(&progress) = SessionProgress::Failed;
                        return;
                    }
                }
                Err(()) => {
                    *lock(&progress) = SessionProgress::Failed;
                    return;
                }
            }
        }

        if ending && !speaking && queue.is_empty() {
            *lock(&progress) = SessionProgress::Idle;
            return;
        }
        *lock(&progress) = session_progress;
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
