//! Provides logging facilities useful for debugging the code.
//!
//! Most importantly there is functionality to easily log whole packets/messages
//! to disk.

// TODO: Evaluate how useful the following loggers would be:
// - Basic console logger, logging only events but not contents.
// - Log rotation.

use messages::MessageInstance;
use packet::{Packet, ReadPacketError};
use types::SequenceNumber;
use util::AtomicU32Counter;

use std;
use std::fs::File;
use std::io::Write;
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

/// Any instance can be used as a logger for this crate.
///
/// If instances are sent over thread boundaries or cloned they are supposed to perform the
/// individual operations of this trait atomically.
pub trait Logger: Send + Sync + 'static {
    fn log_recv(&self, raw_data: &[u8], packet: &Result<Packet, ReadPacketError>);
    fn log_send(&self, seq_num: SequenceNumber, msg: &MessageInstance);

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

    // TODO: Make sure that the buffering performed by std::fs::File is sufficient.
    out_debug: Mutex<File>,
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

    pub fn new<P: Into<PathBuf>>(path: P) -> Result<Self, IoError> {
        let dir = path.into();
        Self::assert_empty_dir(dir.join("recv"))?;
        Self::assert_empty_dir(dir.join("send"))?;

        let log_debug_path = dir.join("debug.log");

        Ok(FullDebugLogger {
            inner: Arc::new(FullDebugLoggerInner {
                dir: dir,
                recv_counter: AtomicU32Counter::new(0),
                send_counter: AtomicU32Counter::new(0),
                out_debug: Mutex::new(File::create(log_debug_path)?),
            }),
        })
    }
}

impl FullDebugLoggerInner {
    fn try_log_recv(
        &self,
        raw_data: &[u8],
        packet: &Result<Packet, ReadPacketError>,
    ) -> Result<(), IoError> {
        let id = self.recv_counter.next();
        let path_bin = self.dir.join(format!("recv/{:08}.bin", id));
        let path_dbg = self.dir.join(format!("recv/{:08}.dbg", id));

        // TODO: consider truncating zero bytes for cleaner output.
        let mut file = File::create(path_bin)?;
        file.write(raw_data)?;

        let mut file = File::create(path_dbg)?;
        writeln!(&mut file, "{:?}", packet)?;
        Ok(())
    }

    fn try_log_send(&self, seq_num: SequenceNumber, msg: &MessageInstance) -> Result<(), IoError> {
        let id = self.send_counter.next();
        let path_dbg = self.dir.join(format!("send/{:08}.dbg", id));

        let mut file = File::create(path_dbg)?;
        writeln!(&mut file, "sequence number: {}", seq_num)?;
        writeln!(&mut file, "message: {:?}", msg)?;
        Ok(())
    }

    fn try_debug(&self, message: &str) -> Result<(), IoError> {
        // TODO: when improving error handling also do this one.
        let mut file = self.out_debug.lock().expect("out_debug mutex is poisoned");
        writeln!(&mut file, "{}", message)
    }
}

// TODO: Better error handling.
impl Logger for FullDebugLogger {
    fn log_recv(&self, raw_data: &[u8], packet: &Result<Packet, ReadPacketError>) {
        self.inner.try_log_recv(raw_data, packet).expect(
            "failed logging packet recv.",
        );
    }

    fn log_send(&self, seq_num: SequenceNumber, msg: &MessageInstance) {
        self.inner.try_log_send(seq_num, msg).expect(
            "failed logging message send.",
        );
    }

    fn debug<S: AsRef<str>>(&self, message: S) {
        self.inner.try_debug(message.as_ref()).expect(
            "failed logging debug log message.",
        );
    }
}

/// This logger can be used in production to avoid the logging overhead completely.
pub struct DiscardLogger;

impl Logger for DiscardLogger {
    fn log_recv(&self, raw_data: &[u8], msg: &Result<Packet, ReadPacketError>) {
        // This method is supposed to do nothing.
    }

    fn log_send(&self, seq_num: SequenceNumber, msg: &MessageInstance) {
        // This method is supposed to do nothing.
    }

    fn debug<S: AsRef<str>>(&self, message: S) {
        // This method is supposed to do nothing.
    }
}
