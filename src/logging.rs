//! Provides logging facilities useful for debugging the code.
//!
//! Most importantly there is functionality to easily log whole packets/messages
//! to disk.

// TODO: Evaluate how useful the following loggers would be:
// - Basic console logger, logging only events but not contents.
// - Log rotation.

use packet::{Packet, ReadPacketError};
use types::SequenceNumber;
use util::AtomicU32Counter;

use std;
use std::fs::File;
use std::io::Write;
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::panic::RefUnwindSafe;
use slog;
use slog::Drain;
use slog_async;
use slog_term;

/// This provides interfaces to the various logs maintained by this crate.
///
/// Design:
/// ======
/// This struct holds internally an Arc to the actual implementation.
/// The implementation is generic, so in the case log data is to be discared,
/// fair performance will be achieved.
///
/// This struct can be cloned and sent across thread boundaries without care.
#[derive(Clone)]
pub struct Log {
    inner: Arc<LogImpl<Ok = (), Err = slog::Never>>,
}

/// Select the minimum level of messages to be included in the log.
///
/// TODO: Add more log levels. In the past there was a discard logger,
///       but maybe errors should always be logged and such functionality
///       would actually be more useful.
#[derive(Clone, Debug)]
pub enum LogLevel {
    /// This logs everything including every received and sent message.
    Debug,
}

impl Log {
    pub fn new_dir<P: Into<PathBuf>>(dir: P, level: LogLevel) -> Result<Self, IoError> {
        let inner = match level {
            LogLevel::Debug => Arc::new(DebugDirLogger::new(dir.into())?),
        };
        Ok(Log { inner: inner })
    }

    /// Returns an instance of `slog::Logger`, which behaves the same way
    /// as if using this struct as Drain directly.
    ///
    /// Note that often the later, using the Drain impl on this struct,
    /// means you don't even need to call this if you already have a `Log`
    /// instance.
    pub fn slog_logger(&self) -> slog::Logger {
        slog::Logger::root(Arc::clone(&self.inner), o!())
    }

    pub fn log_packet_recv(&self, raw_data: &[u8], packet: &Result<Packet, ReadPacketError>) {
        self.inner.log_packet_recv(raw_data, packet)
    }

    pub fn log_packet_send(&self, raw_data: &[u8], packet: &Packet) {
        self.inner.log_packet_send(raw_data, packet)
    }
}

impl slog::Drain for Log {
    type Ok = ();
    type Err = slog::Never;

    fn log(
        &self,
        record: &slog::Record,
        values: &slog::OwnedKVList,
    ) -> Result<Self::Ok, Self::Err> {
        self.inner.log(record, values)
    }
}

trait LogImpl: slog::Drain + LogPacket + Send + Sync + RefUnwindSafe {}

trait LogPacket {
    fn log_packet_recv(&self, raw_data: &[u8], packet: &Result<Packet, ReadPacketError>);
    fn log_packet_send(&self, raw_data: &[u8], packet: &Packet);
}

struct DebugDirLogger {
    /// Target directory.
    dir: PathBuf,

    recv_counter: AtomicU32Counter,
    send_counter: AtomicU32Counter,
    index_recv: Mutex<File>,
    index_send: Mutex<File>,

    logger: slog::Logger,
}

impl LogImpl for DebugDirLogger {}

#[derive(Debug)]
enum LogPacketError {
    Io(IoError),
    FilePoisoned,
}

impl From<IoError> for LogPacketError {
    fn from(err: IoError) -> Self {
        LogPacketError::Io(err)
    }
}

impl<T> From<PoisonError<T>> for LogPacketError {
    fn from(_: PoisonError<T>) -> Self {
        LogPacketError::FilePoisoned
    }
}

impl DebugDirLogger {
    /// Create a new instance of the logger.
    ///
    /// Notice that the logger is expecting an empty directory, if the
    /// directory already contains
    /// other files, it will most likely lead to an error.
    /// So if you want stuff like log rotation, you have to implement it
    /// yourself, but be aware,
    /// that with this uncompressed logging you can quickly accumulate lots of
    /// data.
    fn new(dir: PathBuf) -> Result<Self, IoError> {
        Self::assert_empty_dir(dir.join("recv"))?;
        Self::assert_empty_dir(dir.join("send"))?;

        let index_recv_path = dir.join("recv.index");
        let index_send_path = dir.join("send.index");

        let log_text_path = dir.join("debug.log");
        let log_text_file = File::create(log_text_path)?;
        let decorator = slog_term::PlainDecorator::new(log_text_file);
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();
        let log_text = slog::Logger::root(drain, o!());

        Ok(DebugDirLogger {
            dir: dir,
            recv_counter: AtomicU32Counter::new(0),
            send_counter: AtomicU32Counter::new(0),
            index_recv: Mutex::new(File::create(index_recv_path)?),
            index_send: Mutex::new(File::create(index_send_path)?),
            logger: log_text,
        })
    }

    fn register_id(
        counter: &AtomicU32Counter,
        file_mutex: &Mutex<File>,
        seq_num: Option<SequenceNumber>,
    ) -> Result<u32, LogPacketError> {
        let id = counter.next();
        let mut file = file_mutex.lock()?;
        // TODO: also log the message type for easier identification of relevant entries
        if let Some(seq) = seq_num {
            writeln!(&mut file, "file {:08} => seq {:08}", id, seq)?;
        } else {
            writeln!(&mut file, "file {:08} => error", id)?;
        }
        Ok(id)
    }

    fn assert_empty_dir(dir: PathBuf) -> Result<(), IoError> {
        if !dir.exists() {
            // Create an empty directory and we're fine.
            std::fs::create_dir_all(dir)
        } else if dir.is_dir() {
            // Make sure the directory is empty.
            if std::fs::read_dir(dir)?.next().is_some() {
                Err(IoError::new(IoErrorKind::Other, "Directory is not empty."))
            } else {
                Ok(())
            }
        } else {
            Err(IoError::new(
                IoErrorKind::Other,
                "File exists but is not a directory.",
            ))
        }
    }

    fn try_log_recv(
        &self,
        raw_data: &[u8],
        packet: &Result<Packet, ReadPacketError>,
    ) -> Result<(), LogPacketError> {
        let seq_num = match *packet {
            Ok(ref pkt) => Some(pkt.sequence_number),
            Err(_) => None,
        };
        let id = Self::register_id(&self.recv_counter, &self.index_recv, seq_num)?;
        let path_bin = self.dir.join(format!("recv/{:08}.bin", id));
        let path_txt = self.dir.join(format!("recv/{:08}.txt", id));

        let mut file = File::create(path_bin)?;
        file.write(raw_data)?;

        let mut file = File::create(path_txt)?;
        writeln!(&mut file, "{:?}", packet)?;
        Ok(())
    }

    fn try_log_send(&self, raw_data: &[u8], packet: &Packet) -> Result<(), LogPacketError> {
        let id = Self::register_id(
            &self.send_counter,
            &self.index_send,
            Some(packet.sequence_number),
        )?;
        let path_bin = self.dir.join(format!("send/{:08}.bin", id));
        let path_txt = self.dir.join(format!("send/{:08}.txt", id));

        let mut file = File::create(path_bin)?;
        file.write(raw_data)?;

        let mut file = File::create(path_txt)?;
        writeln!(&mut file, "{:?}", packet)?;
        Ok(())
    }
}

impl slog::Drain for DebugDirLogger {
    type Ok = ();
    type Err = slog::Never;

    fn log(
        &self,
        record: &slog::Record,
        values: &slog::OwnedKVList,
    ) -> Result<Self::Ok, Self::Err> {
        slog::Drain::log(&self.logger, record, values)
    }
}

impl LogPacket for DebugDirLogger {
    fn log_packet_recv(&self, raw_data: &[u8], packet: &Result<Packet, ReadPacketError>) {
        self.try_log_recv(raw_data, packet)
            .expect("failed logging packet recv.");
    }

    fn log_packet_send(&self, raw_data: &[u8], packet: &Packet) {
        self.try_log_send(raw_data, packet)
            .expect("failed logging message send.");
    }
}
