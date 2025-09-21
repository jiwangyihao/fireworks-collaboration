#[cfg(test)]
mod tests {
    use std::time::Duration;
    use crate::core::git::transport::TimingRecorder;

    #[test]
    fn timing_recorder_basic_flow() {
        let mut rec = TimingRecorder::new();
        rec.mark_connect_start();
        std::thread::sleep(Duration::from_millis(5));
        rec.mark_connect_end();
        rec.mark_tls_start();
        std::thread::sleep(Duration::from_millis(5));
        rec.mark_tls_end();
        rec.finish();
        let cap = rec.capture;
        assert!(cap.connect_ms.is_some(), "connect_ms should be recorded");
        assert!(cap.tls_ms.is_some(), "tls_ms should be recorded");
        assert!(cap.total_ms.is_some(), "total_ms should be recorded on finish");
        assert!(cap.total_ms.unwrap() >= cap.connect_ms.unwrap());
    }

    #[test]
    fn finish_idempotent() {
        let mut rec = TimingRecorder::new();
        rec.mark_connect_start(); rec.mark_connect_end();
        rec.finish();
        let first_total = rec.capture.total_ms;
        std::thread::sleep(Duration::from_millis(2));
        rec.finish(); // second call shouldn't change stored total
        assert_eq!(first_total, rec.capture.total_ms, "finish should be idempotent");
    }
}
