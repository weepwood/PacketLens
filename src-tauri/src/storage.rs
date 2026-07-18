use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, Manager};

use crate::model::{
    CaptureRequest, CaptureSessionSummary, CapturedPacket, PacketSummary, StoredPacketSummary,
};

const STORAGE_QUEUE_CAPACITY: usize = 4_096;
const INDEX_BATCH_SIZE: usize = 256;
const FLUSH_INTERVAL: Duration = Duration::from_millis(500);
const MAX_SEGMENT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SEGMENT_AGE: Duration = Duration::from_secs(30 * 60);
const DEFAULT_DISK_QUOTA_BYTES: u64 = 10 * 1024 * 1024 * 1024;

pub struct StoragePacket {
    pub summary: PacketSummary,
    pub packet: CapturedPacket,
}

pub struct StorageRuntime {
    pub session_id: String,
    pub sender: SyncSender<StoragePacket>,
    pub worker: JoinHandle<()>,
}

pub fn start(
    app: &AppHandle,
    request: &CaptureRequest,
    started_at_ms: u64,
    dropped: Arc<AtomicU64>,
) -> Result<StorageRuntime, String> {
    let root = storage_root(app)?;
    fs::create_dir_all(root.join("captures")).map_err(|error| error.to_string())?;

    let database_path = root.join("packetlens.sqlite3");
    let mut connection = open_database(&database_path)?;
    recover_interrupted_sessions(&connection)?;
    enforce_disk_quota(&mut connection, DEFAULT_DISK_QUOTA_BYTES)?;

    let session_id = format!("{started_at_ms}-{}", std::process::id());
    let session_directory = root.join("captures").join(&session_id);
    fs::create_dir_all(&session_directory).map_err(|error| error.to_string())?;

    connection
        .execute(
            "INSERT INTO capture_sessions (
                id, started_at_ms, device_name, capture_filter, status,
                packet_count, byte_count, storage_dropped, segment_count, directory_path
             ) VALUES (?1, ?2, ?3, ?4, 'running', 0, 0, 0, 1, ?5)",
            params![
                session_id,
                started_at_ms as i64,
                request.device_name,
                request.filter,
                session_directory.to_string_lossy()
            ],
        )
        .map_err(|error| error.to_string())?;

    let writer = SessionWriter::new(connection, session_id.clone(), session_directory)?;
    let (sender, receiver) = sync_channel(STORAGE_QUEUE_CAPACITY);
    let worker_dropped = Arc::clone(&dropped);
    let worker = thread::Builder::new()
        .name("packetlens-storage".to_string())
        .spawn(move || run_storage_worker(writer, receiver, worker_dropped))
        .map_err(|error| error.to_string())?;

    Ok(StorageRuntime {
        session_id,
        sender,
        worker,
    })
}

pub fn list_sessions(
    app: &AppHandle,
    limit: u32,
    offset: u32,
) -> Result<Vec<CaptureSessionSummary>, String> {
    let database_path = storage_root(app)?.join("packetlens.sqlite3");
    if !database_path.exists() {
        return Ok(Vec::new());
    }

    let connection = open_database(&database_path)?;
    let mut statement = connection
        .prepare(
            "SELECT id, started_at_ms, ended_at_ms, device_name, capture_filter, status,
                    packet_count, byte_count, storage_dropped, segment_count, directory_path,
                    last_error
             FROM capture_sessions
             ORDER BY started_at_ms DESC
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(|error| error.to_string())?;

    let sessions = statement
        .query_map(params![limit.clamp(1, 200), offset], |row| {
            Ok(CaptureSessionSummary {
                id: row.get(0)?,
                started_at_ms: row.get::<_, i64>(1)? as u64,
                ended_at_ms: row.get::<_, Option<i64>>(2)?.map(|value| value as u64),
                device_name: row.get(3)?,
                filter: row.get(4)?,
                status: row.get(5)?,
                packet_count: row.get::<_, i64>(6)? as u64,
                byte_count: row.get::<_, i64>(7)? as u64,
                storage_dropped: row.get::<_, i64>(8)? as u64,
                segment_count: row.get::<_, i64>(9)? as u32,
                directory_path: row.get(10)?,
                last_error: row.get(11)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    Ok(sessions)
}

pub fn list_session_packets(
    app: &AppHandle,
    session_id: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<StoredPacketSummary>, String> {
    let database_path = storage_root(app)?.join("packetlens.sqlite3");
    if !database_path.exists() {
        return Ok(Vec::new());
    }

    let connection = open_database(&database_path)?;
    let mut statement = connection
        .prepare(
            "SELECT packet_id, timestamp_micros, segment_index, file_offset,
                    captured_length, original_length, protocol, source, destination,
                    source_port, destination_port, process_id, process_name, info
             FROM packet_index
             WHERE session_id = ?1
             ORDER BY packet_id ASC
             LIMIT ?2 OFFSET ?3",
        )
        .map_err(|error| error.to_string())?;

    let packets = statement
        .query_map(params![session_id, limit.clamp(1, 1_000), offset], |row| {
            Ok(StoredPacketSummary {
                id: row.get::<_, i64>(0)? as u64,
                timestamp_micros: row.get::<_, i64>(1)? as u64,
                segment_index: row.get::<_, i64>(2)? as u32,
                file_offset: row.get::<_, i64>(3)? as u64,
                captured_length: row.get::<_, i64>(4)? as u32,
                length: row.get::<_, i64>(5)? as u32,
                protocol: row.get(6)?,
                source: row.get(7)?,
                destination: row.get(8)?,
                source_port: row.get::<_, Option<i64>>(9)?.map(|value| value as u16),
                destination_port: row.get::<_, Option<i64>>(10)?.map(|value| value as u16),
                process_id: row.get::<_, Option<i64>>(11)?.map(|value| value as u32),
                process_name: row.get(12)?,
                info: row.get(13)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    Ok(packets)
}

pub fn delete_session(app: &AppHandle, session_id: &str) -> Result<(), String> {
    let database_path = storage_root(app)?.join("packetlens.sqlite3");
    if !database_path.exists() {
        return Ok(());
    }

    let connection = open_database(&database_path)?;
    let directory: Option<String> = connection
        .query_row(
            "SELECT directory_path FROM capture_sessions WHERE id = ?1 AND status != 'running'",
            params![session_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let Some(directory) = directory else {
        return Err("捕获会话不存在或仍在运行".to_string());
    };

    let directory = PathBuf::from(directory);
    if directory.exists() {
        fs::remove_dir_all(&directory).map_err(|error| error.to_string())?;
    }
    connection
        .execute(
            "DELETE FROM capture_sessions WHERE id = ?1",
            params![session_id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn run_storage_worker(
    mut writer: SessionWriter,
    receiver: Receiver<StoragePacket>,
    dropped: Arc<AtomicU64>,
) {
    let result = loop {
        match receiver.recv_timeout(FLUSH_INTERVAL) {
            Ok(packet) => {
                if let Err(error) = writer.write_packet(packet) {
                    break Err(error);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Err(error) = writer.flush() {
                    break Err(error);
                }
            }
            Err(RecvTimeoutError::Disconnected) => break Ok(()),
        }
    };

    let dropped = dropped.load(Ordering::Relaxed);
    match result {
        Ok(()) => {
            let _ = writer.finish("completed", dropped, None);
        }
        Err(error) => {
            let _ = writer.finish("error", dropped, Some(&error));
        }
    }
}

struct SessionWriter {
    connection: Connection,
    session_id: String,
    session_directory: PathBuf,
    segment_index: u32,
    segment_started: Instant,
    file: BufWriter<File>,
    bytes_written: u64,
    segment_packets: u64,
    total_packets: u64,
    total_bytes: u64,
    pending_rows: Vec<IndexRow>,
    last_flush: Instant,
}

impl SessionWriter {
    fn new(
        connection: Connection,
        session_id: String,
        session_directory: PathBuf,
    ) -> Result<Self, String> {
        let (file, bytes_written) = create_segment(&session_directory, 0)?;
        Ok(Self {
            connection,
            session_id,
            session_directory,
            segment_index: 0,
            segment_started: Instant::now(),
            file,
            bytes_written,
            segment_packets: 0,
            total_packets: 0,
            total_bytes: 0,
            pending_rows: Vec::with_capacity(INDEX_BATCH_SIZE),
            last_flush: Instant::now(),
        })
    }

    fn write_packet(&mut self, packet: StoragePacket) -> Result<(), String> {
        let block = enhanced_packet_block(&packet.packet);
        if self.segment_packets > 0
            && (self.bytes_written.saturating_add(block.len() as u64) > MAX_SEGMENT_BYTES
                || self.segment_started.elapsed() >= MAX_SEGMENT_AGE)
        {
            self.rotate_segment()?;
        }

        let file_offset = self.bytes_written;
        self.file
            .write_all(&block)
            .map_err(|error| error.to_string())?;
        self.bytes_written = self.bytes_written.saturating_add(block.len() as u64);
        self.segment_packets = self.segment_packets.saturating_add(1);
        self.total_packets = self.total_packets.saturating_add(1);
        self.total_bytes = self
            .total_bytes
            .saturating_add(packet.packet.original_length as u64);

        self.pending_rows.push(IndexRow {
            summary: packet.summary,
            segment_index: self.segment_index,
            file_offset,
            captured_length: packet.packet.data.len() as u32,
        });

        if self.pending_rows.len() >= INDEX_BATCH_SIZE
            || self.last_flush.elapsed() >= FLUSH_INTERVAL
        {
            self.flush()?;
        }
        Ok(())
    }

    fn rotate_segment(&mut self) -> Result<(), String> {
        self.flush()?;
        self.segment_index = self.segment_index.saturating_add(1);
        let (file, bytes_written) = create_segment(&self.session_directory, self.segment_index)?;
        self.file = file;
        self.bytes_written = bytes_written;
        self.segment_packets = 0;
        self.segment_started = Instant::now();
        self.connection
            .execute(
                "UPDATE capture_sessions SET segment_count = ?2 WHERE id = ?1",
                params![self.session_id, self.segment_index as i64 + 1],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        if !self.pending_rows.is_empty() {
            let rows = std::mem::take(&mut self.pending_rows);
            let transaction = self
                .connection
                .transaction()
                .map_err(|error| error.to_string())?;
            {
                let mut statement = transaction
                    .prepare_cached(
                        "INSERT INTO packet_index (
                            session_id, packet_id, timestamp_micros, segment_index, file_offset,
                            captured_length, original_length, protocol, source, destination,
                            source_port, destination_port, process_id, process_name, info
                         ) VALUES (
                            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
                         )",
                    )
                    .map_err(|error| error.to_string())?;
                for row in rows {
                    statement
                        .execute(params![
                            self.session_id,
                            row.summary.id as i64,
                            row.summary.timestamp_micros as i64,
                            row.segment_index as i64,
                            row.file_offset as i64,
                            row.captured_length as i64,
                            row.summary.length as i64,
                            row.summary.protocol,
                            row.summary.source,
                            row.summary.destination,
                            row.summary.source_port.map(i64::from),
                            row.summary.destination_port.map(i64::from),
                            row.summary.process_id.map(i64::from),
                            row.summary.process_name,
                            row.summary.info
                        ])
                        .map_err(|error| error.to_string())?;
                }
            }
            transaction.commit().map_err(|error| error.to_string())?;
        }

        self.file.flush().map_err(|error| error.to_string())?;
        self.connection
            .execute(
                "UPDATE capture_sessions
                 SET packet_count = ?2, byte_count = ?3
                 WHERE id = ?1",
                params![
                    self.session_id,
                    self.total_packets as i64,
                    self.total_bytes as i64
                ],
            )
            .map_err(|error| error.to_string())?;
        self.last_flush = Instant::now();
        Ok(())
    }

    fn finish(
        &mut self,
        status: &str,
        dropped: u64,
        last_error: Option<&str>,
    ) -> Result<(), String> {
        let flush_error = self.flush().err();
        let final_error = last_error
            .map(str::to_string)
            .or(flush_error)
            .unwrap_or_default();
        let final_status = if final_error.is_empty() {
            status
        } else {
            "error"
        };

        self.connection
            .execute(
                "UPDATE capture_sessions
                 SET ended_at_ms = ?2, status = ?3, packet_count = ?4, byte_count = ?5,
                     storage_dropped = ?6, segment_count = ?7, last_error = ?8
                 WHERE id = ?1",
                params![
                    self.session_id,
                    unix_time_ms() as i64,
                    final_status,
                    self.total_packets as i64,
                    self.total_bytes as i64,
                    dropped as i64,
                    self.segment_index as i64 + 1,
                    if final_error.is_empty() {
                        None::<String>
                    } else {
                        Some(final_error)
                    }
                ],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

struct IndexRow {
    summary: PacketSummary,
    segment_index: u32,
    file_offset: u64,
    captured_length: u32,
}

fn create_segment(directory: &Path, index: u32) -> Result<(BufWriter<File>, u64), String> {
    let path = directory.join(format!("segment-{index:04}.pcapng"));
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut writer = BufWriter::new(file);
    let headers = pcapng_headers();
    writer
        .write_all(&headers)
        .map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())?;
    Ok((writer, headers.len() as u64))
}

fn pcapng_headers() -> Vec<u8> {
    let mut output = Vec::with_capacity(48);

    push_u32(&mut output, 0x0A0D_0D0A);
    push_u32(&mut output, 28);
    push_u32(&mut output, 0x1A2B_3C4D);
    push_u16(&mut output, 1);
    push_u16(&mut output, 0);
    output.extend_from_slice(&u64::MAX.to_le_bytes());
    push_u32(&mut output, 28);

    push_u32(&mut output, 1);
    push_u32(&mut output, 20);
    push_u16(&mut output, 1);
    push_u16(&mut output, 0);
    push_u32(&mut output, 65_535);
    push_u32(&mut output, 20);

    output
}

fn enhanced_packet_block(packet: &CapturedPacket) -> Vec<u8> {
    let padding = (4 - packet.data.len() % 4) % 4;
    let total_length = 32 + packet.data.len() + padding;
    let timestamp_high = (packet.timestamp_micros >> 32) as u32;
    let timestamp_low = packet.timestamp_micros as u32;

    let mut output = Vec::with_capacity(total_length);
    push_u32(&mut output, 6);
    push_u32(&mut output, total_length as u32);
    push_u32(&mut output, 0);
    push_u32(&mut output, timestamp_high);
    push_u32(&mut output, timestamp_low);
    push_u32(&mut output, packet.data.len() as u32);
    push_u32(&mut output, packet.original_length);
    output.extend_from_slice(&packet.data);
    output.resize(output.len() + padding, 0);
    push_u32(&mut output, total_length as u32);
    output
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn storage_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|error| error.to_string())
}

fn open_database(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS capture_sessions (
                 id TEXT PRIMARY KEY,
                 started_at_ms INTEGER NOT NULL,
                 ended_at_ms INTEGER,
                 device_name TEXT NOT NULL,
                 capture_filter TEXT,
                 status TEXT NOT NULL,
                 packet_count INTEGER NOT NULL DEFAULT 0,
                 byte_count INTEGER NOT NULL DEFAULT 0,
                 storage_dropped INTEGER NOT NULL DEFAULT 0,
                 segment_count INTEGER NOT NULL DEFAULT 1,
                 directory_path TEXT NOT NULL,
                 last_error TEXT
             );
             CREATE TABLE IF NOT EXISTS packet_index (
                 session_id TEXT NOT NULL,
                 packet_id INTEGER NOT NULL,
                 timestamp_micros INTEGER NOT NULL,
                 segment_index INTEGER NOT NULL,
                 file_offset INTEGER NOT NULL,
                 captured_length INTEGER NOT NULL,
                 original_length INTEGER NOT NULL,
                 protocol TEXT NOT NULL,
                 source TEXT NOT NULL,
                 destination TEXT NOT NULL,
                 source_port INTEGER,
                 destination_port INTEGER,
                 process_id INTEGER,
                 process_name TEXT,
                 info TEXT NOT NULL,
                 PRIMARY KEY (session_id, packet_id),
                 FOREIGN KEY (session_id) REFERENCES capture_sessions(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS idx_packet_session_time
                 ON packet_index(session_id, timestamp_micros);
             CREATE INDEX IF NOT EXISTS idx_packet_session_protocol
                 ON packet_index(session_id, protocol);",
        )
        .map_err(|error| error.to_string())?;
    Ok(connection)
}

fn recover_interrupted_sessions(connection: &Connection) -> Result<(), String> {
    connection
        .execute(
            "UPDATE capture_sessions
             SET status = 'interrupted', ended_at_ms = ?1,
                 last_error = COALESCE(last_error, '应用在会话完成前退出')
             WHERE status = 'running'",
            params![unix_time_ms() as i64],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn enforce_disk_quota(connection: &mut Connection, quota: u64) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "SELECT id, directory_path
             FROM capture_sessions
             WHERE status != 'running'
             ORDER BY started_at_ms ASC",
        )
        .map_err(|error| error.to_string())?;
    let sessions = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);

    let mut entries = Vec::with_capacity(sessions.len());
    let mut total = 0u64;
    for (id, directory) in sessions {
        let size = directory_size(Path::new(&directory));
        total = total.saturating_add(size);
        entries.push((id, PathBuf::from(directory), size));
    }

    let target = quota.saturating_mul(9) / 10;
    for (id, directory, size) in entries {
        if total <= quota {
            break;
        }
        if directory.exists() && fs::remove_dir_all(&directory).is_err() {
            continue;
        }
        connection
            .execute("DELETE FROM capture_sessions WHERE id = ?1", params![id])
            .map_err(|error| error.to_string())?;
        total = total.saturating_sub(size);
        if total <= target {
            break;
        }
    }
    Ok(())
}

fn directory_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::metadata(path) else {
        return 0;
    };
    if metadata.is_file() {
        return metadata.len();
    }

    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| directory_size(&entry.path()))
        .fold(0u64, u64::saturating_add)
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcapng_headers_have_valid_block_lengths() {
        let headers = pcapng_headers();
        assert_eq!(headers.len(), 48);
        assert_eq!(u32::from_le_bytes(headers[4..8].try_into().unwrap()), 28);
        assert_eq!(u32::from_le_bytes(headers[24..28].try_into().unwrap()), 28);
        assert_eq!(u32::from_le_bytes(headers[32..36].try_into().unwrap()), 20);
        assert_eq!(u32::from_le_bytes(headers[44..48].try_into().unwrap()), 20);
    }

    #[test]
    fn enhanced_packet_block_is_padded_and_self_describing() {
        let packet = CapturedPacket {
            timestamp_micros: 123,
            original_length: 3,
            data: vec![1, 2, 3],
        };
        let block = enhanced_packet_block(&packet);
        assert_eq!(block.len(), 36);
        assert_eq!(u32::from_le_bytes(block[4..8].try_into().unwrap()), 36);
        assert_eq!(u32::from_le_bytes(block[32..36].try_into().unwrap()), 36);
    }
}
