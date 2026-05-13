//! PTY backend powered by `portable-pty`. Bridges its blocking Read/Write to
//! async via `spawn_blocking` adapters.

use std::io::{Read, Write};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::{mpsc, Mutex};

use crate::{BackgroundCommandHandle, ChildHandle, PtyError};

/// Wrap a `std::io::Read` (blocking) as a tokio `AsyncRead` by pulling bytes
/// on a dedicated blocking task and shuttling them via an unbounded channel.
pub struct BlockingReaderToAsync {
    rx: mpsc::UnboundedReceiver<std::io::Result<Vec<u8>>>,
    pending: Option<Vec<u8>>,
    cursor: usize,
}

impl BlockingReaderToAsync {
    pub fn new<R: Read + Send + 'static>(mut reader: R) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::task::spawn_blocking(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(Ok(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        break;
                    }
                }
            }
        });
        Self {
            rx,
            pending: None,
            cursor: 0,
        }
    }
}

impl AsyncRead for BlockingReaderToAsync {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.pending.is_none() {
            match self.rx.poll_recv(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Err(e)),
                Poll::Ready(Some(Ok(bytes))) => {
                    self.pending = Some(bytes);
                    self.cursor = 0;
                }
            }
        }
        let buf = self.pending.as_ref().unwrap();
        let remaining = &buf[self.cursor..];
        let n = remaining.len().min(out.remaining());
        out.put_slice(&remaining[..n]);
        let new_cursor = self.cursor + n;
        if new_cursor >= buf.len() {
            self.pending = None;
            self.cursor = 0;
        } else {
            self.cursor = new_cursor;
        }
        Poll::Ready(Ok(()))
    }
}

/// Wrap a `std::io::Write` (blocking) as a tokio `AsyncWrite`.
pub struct BlockingWriterToAsync {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl BlockingWriterToAsync {
    pub fn new<W: Write + Send + 'static>(writer: W) -> Self {
        Self {
            writer: Arc::new(Mutex::new(Box::new(writer))),
        }
    }
}

impl AsyncWrite for BlockingWriterToAsync {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let writer = self.writer.clone();
        let bytes = buf.to_vec();
        // Synchronously try to acquire the lock; if contended, spin-wait briefly.
        // PTY writes are short; this matches portable-pty's intended use.
        let mut guard = match writer.try_lock() {
            Ok(g) => g,
            Err(_) => return Poll::Pending,
        };
        let n = guard.write(&bytes)?;
        Poll::Ready(Ok(n))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let writer = self.writer.clone();
        let mut guard = match writer.try_lock() {
            Ok(g) => g,
            Err(_) => return Poll::Pending,
        };
        guard.flush()?;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.poll_flush(cx)
    }
}

/// Spawn `cmd` inside a PTY of (rows × cols). Mirrors `spawn_with_pgrp`'s
/// contract but goes through `portable-pty`'s `PtySystem`.
pub fn spawn_with_pty(
    cmd: portable_pty::CommandBuilder,
    rows: u16,
    cols: u16,
) -> Result<BackgroundCommandHandle, PtyError> {
    let pty_system = portable_pty::native_pty_system();
    let pair = pty_system
        .openpty(portable_pty::PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| PtyError::PgrpCapture(format!("openpty: {e}")))?;

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| PtyError::PgrpCapture(format!("spawn_command: {e}")))?;
    drop(pair.slave);

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| PtyError::PgrpCapture(format!("try_clone_reader: {e}")))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| PtyError::PgrpCapture(format!("take_writer: {e}")))?;

    let pid = child.process_id();
    let pgid = pid.and_then(|p| {
        #[cfg(unix)]
        unsafe {
            let g = libc::getpgid(p as i32);
            if g < 0 {
                None
            } else {
                Some(g as u32)
            }
        }
        #[cfg(not(unix))]
        {
            let _ = p;
            None
        }
    });

    Ok(BackgroundCommandHandle {
        child: ChildHandle::Pty {
            master: Arc::new(Mutex::new(pair.master)),
            child: Arc::new(Mutex::new(child)),
            pgid,
        },
        stdout: Box::new(BlockingReaderToAsync::new(reader)) as _,
        stderr: None,
        stdin: Some(Box::new(BlockingWriterToAsync::new(writer)) as _),
        pgid,
    })
}
