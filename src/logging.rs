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

/// Any instance can be used as a logger for this crate.
///
/// If instances are sent over thread boundaries or cloned they are supposed to perform the
/// individual operations of this trait atomically.
pub trait Logger: Clone + Send + Sync + 'static {
    fn log_recv(&self, raw_data: &[u8], packet: &Result<Packet, ReadPacketError>);
    fn log_send(&self, raw_data: &[u8], packet: &Packet);

    fn debug<S: AsRef<str>>(&self, message: S);
    fn info<S: AsRef<str>>(&self, message: S) {
        self.debug(message); // TODO fix placeholder later
    }
}

/// A logger to be used to extract the full debug information into a specified directory.
///
/// Note that the numbering of the individual files doesn't correspond directly to the sequence
/// numbers, since it's possible for packets to be transmitted multiple times with the same
/// sequence number.
#[derive(Clone)]
pub struct FullDebugLogger {
    inner: Arc<FullDebugLoggerInner>,
}

struct FullDebugLoggerInner {
    /// The directory that is to be logged to.
    dir: PathBuf,

    recv_counter: AtomicU32Counter,
    send_counter: AtomicU32Counter,

    index_recv: Mutex<File>,
    index_send: Mutex<File>,

    // TODO: Make sure that the buffering performed by std::fs::File is sufficient.
    out_debug: Mutex<File>,
}

#[derive(Debug)]
enum FullDebugLoggerError {
    Io(IoError),
    FilePoisoned,
}

impl From<IoError> for FullDebugLoggerError {
    fn from(err: IoError) -> Self {
        FullDebugLoggerError::Io(err)
    }
}

impl<T> From<PoisonError<T>> for FullDebugLoggerError {
    fn from(_: PoisonError<T>) -> Self {
        FullDebugLoggerError::FilePoisoned
    }
}

impl FullDebugLogger {
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

    /// Create a new instance of the logger.
    ///
    /// Notice that the logger is expecting an empty directory, if the directory already contains
    /// other files, it will most likely lead to an error.
    /// So if you want stuff like log rotation, you have to implement it yourself, but be aware,
    /// that with this uncompressed logging you can quickly accumulate lots of data.
    pub fn new<P: Into<PathBuf>>(path: P) -> Result<Self, IoError> {
        let dir = path.into();
        Self::assert_empty_dir(dir.join("recv"))?;
        Self::assert_empty_dir(dir.join("send"))?;

        let index_recv_path = dir.join("recv.index");
        let index_send_path = dir.join("send.index");
        let log_debug_path = dir.join("debug.log");

        Ok(FullDebugLogger {
            inner: Arc::new(FullDebugLoggerInner {
                dir: dir,
                recv_counter: AtomicU32Counter::new(0),
                send_counter: AtomicU32Counter::new(0),
                index_recv: Mutex::new(File::create(index_recv_path)?),
                index_send: Mutex::new(File::create(index_send_path)?),
                out_debug: Mutex::new(File::create(log_debug_path)?),
            }),
        })
    }
}

impl FullDebugLoggerInner {
    fn register_id(
        counter: &AtomicU32Counter,
        file_mutex: &Mutex<File>,
        seq_num: Option<SequenceNumber>,
    ) -> Result<u32, FullDebugLoggerError> {
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

    fn try_log_recv(
        &self,
        raw_data: &[u8],
        packet: &Result<Packet, ReadPacketError>,
    ) -> Result<(), FullDebugLoggerError> {
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

    fn try_log_send(&self, raw_data: &[u8], packet: &Packet) -> Result<(), FullDebugLoggerError> {
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

    fn try_debug(&self, message: &str) -> Result<(), FullDebugLoggerError> {
        let mut file = self.out_debug.lock()?;
        writeln!(&mut file, "{}", message)?;
        Ok(())
    }
}

// TODO: Better error handling.
impl Logger for FullDebugLogger {
    fn log_recv(&self, raw_data: &[u8], packet: &Result<Packet, ReadPacketError>) {
        self.inner
            .try_log_recv(raw_data, packet)
            .expect("failed logging packet recv.");
    }

    fn log_send(&self, raw_data: &[u8], packet: &Packet) {
        self.inner
            .try_log_send(raw_data, packet)
            .expect("failed logging message send.");
    }

    fn debug<S: AsRef<str>>(&self, message: S) {
        self.inner
            .try_debug(message.as_ref())
            .expect("failed logging debug log message.");
    }
}

/// This logger can be used in production to avoid the logging overhead completely.
#[derive(Clone)]
pub struct DiscardLogger;

impl Logger for DiscardLogger {
    fn log_recv(&self, _: &[u8], _: &Result<Packet, ReadPacketError>) {
        // This method is supposed to do nothing.
    }

    fn log_send(&self, _: &[u8], _: &Packet) {
        // This method is supposed to do nothing.
    }

    fn debug<S: AsRef<str>>(&self, _: S) {
        // This method is supposed to do nothing.
    }
}
