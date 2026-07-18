use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{sync_channel, RecvTimeoutError, TrySendError},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Emitter};

use crate::{
    model::{CaptureRequest, CaptureStatistics, CaptureStatus, PacketBatch, PacketSummary},
    process::ProcessResolver,
    protocol,
};

use super::npcap::CaptureSession;

#[derive(Default)]
struct Counters {
    packets: AtomicU64,
    bytes: AtomicU64,
    dropped: AtomicU64,
}

pub struct CaptureService {
    running: Arc<AtomicBool>,
    status: Arc<RwLock<CaptureStatus>>,
    workers: Mutex<Vec<JoinHandle<()>>>,
}

impl Default for CaptureService {
    fn default() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            status: Arc::new(RwLock::new(CaptureStatus::default())),
            workers: Mutex::new(Vec::new()),
        }
    }
}

impl CaptureService {
    pub fn status(&self) -> CaptureStatus {
        self.status
            .read()
            .expect("capture status lock poisoned")
            .clone()
    }

    pub fn start(&self, app: AppHandle, request: CaptureRequest) -> Result<CaptureStatus, String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err("抓包任务已经在运行".to_string());
        }

        let session = match CaptureSession::open(&request) {
            Ok(session) => session,
            Err(error) => {
                self.running.store(false, Ordering::SeqCst);
                self.set_error(error.clone());
                return Err(error);
            }
        };

        let started_at_unix_ms = unix_time_ms();
        {
            let mut status = self.status.write().expect("capture status lock poisoned");
            *status = CaptureStatus {
                running: true,
                device_name: Some(request.device_name.clone()),
                started_at_unix_ms: Some(started_at_unix_ms),
                npcap_available: true,
                last_error: None,
            };
        }

        let (sender, receiver) = sync_channel::<PacketSummary>(8_192);
        let counters = Arc::new(Counters::default());
        let next_id = Arc::new(AtomicU64::new(1));

        let capture_running = Arc::clone(&self.running);
        let capture_status = Arc::clone(&self.status);
        let capture_counters = Arc::clone(&counters);
        let capture_ids = Arc::clone(&next_id);
        let capture_worker = thread::Builder::new()
            .name("packetlens-capture".to_string())
            .spawn(move || {
                let mut session = session;
                let mut resolver = ProcessResolver::new();

                while capture_running.load(Ordering::Relaxed) {
                    match session.next_packet() {
                        Ok(Some(packet)) => {
                            let original_length = packet.original_length as u64;
                            let id = capture_ids.fetch_add(1, Ordering::Relaxed);
                            let mut summary = protocol::summarize(id, packet);
                            resolver.enrich(&mut summary);

                            capture_counters.packets.fetch_add(1, Ordering::Relaxed);
                            capture_counters
                                .bytes
                                .fetch_add(original_length, Ordering::Relaxed);

                            match sender.try_send(summary) {
                                Ok(()) => {}
                                Err(TrySendError::Full(_)) => {
                                    capture_counters.dropped.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(TrySendError::Disconnected(_)) => break,
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            capture_running.store(false, Ordering::SeqCst);
                            let mut status = capture_status
                                .write()
                                .expect("capture status lock poisoned");
                            status.running = false;
                            status.last_error = Some(error);
                            break;
                        }
                    }
                }
            })
            .map_err(|error| {
                self.running.store(false, Ordering::SeqCst);
                error.to_string()
            })?;

        let batch_counters = Arc::clone(&counters);
        let batch_worker = thread::Builder::new()
            .name("packetlens-batch".to_string())
            .spawn(move || {
                let mut last_sample = Instant::now();
                let mut last_packets = 0u64;
                let mut last_bytes = 0u64;

                loop {
                    let mut packets = Vec::with_capacity(512);
                    match receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(packet) => packets.push(packet),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                    packets.extend(receiver.try_iter().take(2_048));

                    let now = Instant::now();
                    let elapsed = now.duration_since(last_sample).as_secs_f64().max(0.001);
                    let captured_packets = batch_counters.packets.load(Ordering::Relaxed);
                    let captured_bytes = batch_counters.bytes.load(Ordering::Relaxed);
                    let statistics = CaptureStatistics {
                        captured_packets,
                        captured_bytes,
                        dropped_packets: batch_counters.dropped.load(Ordering::Relaxed),
                        packets_per_second: ((captured_packets - last_packets) as f64 / elapsed)
                            .round() as u64,
                        bytes_per_second: ((captured_bytes - last_bytes) as f64 / elapsed).round()
                            as u64,
                    };

                    last_sample = now;
                    last_packets = captured_packets;
                    last_bytes = captured_bytes;

                    let _ = app.emit(
                        "packet-batch",
                        PacketBatch {
                            packets,
                            statistics,
                        },
                    );
                }
            })
            .map_err(|error| {
                self.running.store(false, Ordering::SeqCst);
                error.to_string()
            })?;

        let mut workers = self.workers.lock().expect("capture workers lock poisoned");
        workers.push(capture_worker);
        workers.push(batch_worker);
        Ok(self.status())
    }

    pub fn stop(&self) -> CaptureStatus {
        self.running.store(false, Ordering::SeqCst);

        let workers = {
            let mut guard = self.workers.lock().expect("capture workers lock poisoned");
            std::mem::take(&mut *guard)
        };
        for worker in workers {
            let _ = worker.join();
        }

        {
            let mut status = self.status.write().expect("capture status lock poisoned");
            status.running = false;
        }
        self.status()
    }

    fn set_error(&self, error: String) {
        let mut status = self.status.write().expect("capture status lock poisoned");
        status.running = false;
        status.last_error = Some(error);
        status.npcap_available = super::npcap_available();
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
