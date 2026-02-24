use std::path::PathBuf;

use crate::llm::provider::ProcessingMode;

/// A recording that was captured while the device was offline and is waiting
/// to be processed once connectivity is restored.
#[derive(Debug, Clone)]
pub struct QueuedRecording {
    pub session_id: String,
    pub audio_path: PathBuf,
    pub mode: ProcessingMode,
    pub target_language: Option<String>,
    pub selected_text: Option<String>,
    pub app_context: Option<String>,
}

/// A simple in-memory FIFO queue of recordings captured while offline.
///
/// When the network comes back, the orchestrator can call [`drain`](Self::drain)
/// to retrieve all pending items and re-submit them through the pipeline.
pub struct OfflineQueue {
    items: Vec<QueuedRecording>,
}

impl OfflineQueue {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Append a recording to the end of the queue.
    pub fn enqueue(&mut self, recording: QueuedRecording) {
        self.items.push(recording);
    }

    /// Remove **all** queued recordings and return them in FIFO order.
    pub fn drain(&mut self) -> Vec<QueuedRecording> {
        std::mem::take(&mut self.items)
    }

    /// Number of recordings currently waiting in the queue.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` when there are no queued recordings.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for OfflineQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recording(id: &str) -> QueuedRecording {
        QueuedRecording {
            session_id: id.to_string(),
            audio_path: PathBuf::from(format!("/tmp/{id}.wav")),
            mode: ProcessingMode::Dictate,
            target_language: None,
            selected_text: None,
            app_context: None,
        }
    }

    #[test]
    fn new_queue_is_empty() {
        let q = OfflineQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn enqueue_increases_length() {
        let mut q = OfflineQueue::new();
        q.enqueue(sample_recording("a"));
        assert_eq!(q.len(), 1);
        assert!(!q.is_empty());

        q.enqueue(sample_recording("b"));
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn drain_returns_all_items_in_order() {
        let mut q = OfflineQueue::new();
        q.enqueue(sample_recording("1"));
        q.enqueue(sample_recording("2"));
        q.enqueue(sample_recording("3"));

        let items = q.drain();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].session_id, "1");
        assert_eq!(items[1].session_id, "2");
        assert_eq!(items[2].session_id, "3");

        // Queue should be empty after drain.
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn drain_on_empty_queue_returns_empty_vec() {
        let mut q = OfflineQueue::new();
        let items = q.drain();
        assert!(items.is_empty());
    }

    #[test]
    fn enqueue_after_drain_works() {
        let mut q = OfflineQueue::new();
        q.enqueue(sample_recording("x"));
        let _ = q.drain();

        q.enqueue(sample_recording("y"));
        assert_eq!(q.len(), 1);
        let items = q.drain();
        assert_eq!(items[0].session_id, "y");
    }
}
