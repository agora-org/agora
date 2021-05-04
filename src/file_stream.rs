use crate::{
  error::{self, Result},
  file_path::FilePath,
};
use futures::Stream;
use hyper::body::Bytes;
use pin_project::pin_project;
use snafu::ResultExt;
use std::{
  mem::MaybeUninit,
  pin::Pin,
  task::{self, Poll},
};
use tokio::{
  fs::File,
  io::{AsyncRead, ReadBuf},
};

#[pin_project]
pub(crate) struct FileStream {
  #[pin]
  file: File,
  path: FilePath,
}

impl FileStream {
  pub(crate) async fn new(file_path: FilePath) -> Result<Self> {
    Ok(Self {
      file: File::open(&file_path)
        .await
        .with_context(|| error::FileIo {
          path: file_path.clone(),
        })?,
      path: file_path,
    })
  }
}

impl Stream for FileStream {
  type Item = Result<Bytes>;

  fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
    let data = &mut [MaybeUninit::uninit(); 16 * 1024];
    let mut buf = ReadBuf::uninit(data);

    let projected = self.project();

    let file = projected.file;
    let path = projected.path;

    let poll = file
      .poll_read(cx, &mut buf)
      .map(|result| result.with_context(|| error::FileIo { path: path.clone() }))?;

    if poll.is_pending() {
      return Poll::Pending;
    }

    if buf.filled().is_empty() {
      return Poll::Ready(None);
    }

    Poll::Ready(Some(Ok(Bytes::copy_from_slice(buf.filled()))))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::StreamExt;
  use std::ffi::CString;
  use tokio::io::AsyncWriteExt;

  #[tokio::test]
  async fn file_stream_yields_file_contents() {
    let tempdir = tempfile::tempdir().unwrap();
    let dir = tempdir.path();
    let file_path = FilePath::new_unchecked(&dir, "foo.txt");

    let input = &[0x15; 200];

    std::fs::write(&file_path, input).unwrap();

    let mut stream = FileStream::new(file_path).await.unwrap();

    let mut output = Vec::new();

    while let Some(result) = stream.next().await {
      let bytes = result.unwrap();
      output.extend(bytes);
    }

    assert_eq!(output, input);
  }

  #[tokio::test]
  async fn file_stream_works_on_fifos() {
    let tempdir = tempfile::tempdir().unwrap();
    let dir = tempdir.path();
    let fifo_path = dir.join("fifo");
    let fifo_c = CString::new(fifo_path.to_string_lossy().into_owned()).unwrap();

    assert_eq!(unsafe { libc::mkfifo(fifo_c.as_ptr(), libc::S_IRWXU) }, 0);

    let mut fifo = File::open(&fifo_path).await.unwrap();

    tokio::spawn(async move {
      fifo.write_all(b"hello\n").await.unwrap();
    });

    let mut stream = FileStream::new(FilePath::new_unchecked(&dir, "fifo"))
      .await
      .unwrap();

    let mut output = Vec::new();

    while let Some(result) = stream.next().await {
      let bytes = result.unwrap();
      output.extend(bytes);
    }

    assert_eq!(output, b"hello");
  }

  #[test]
  fn can_read_and_write_fifos() {
    let tempdir = tempfile::tempdir().unwrap();
    let dir = tempdir.path();
    let fifo_path = dir.join("fifo");
    let fifo_c = CString::new(fifo_path.to_string_lossy().into_owned()).unwrap();

    assert_eq!(unsafe { libc::mkfifo(fifo_c.as_ptr(), libc::S_IRWXU) }, 0);

    let mut writer = {
      let fifo_path = fifo_path.clone();
      std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("echo hello > {}", fifo_path.display()))
        .spawn()
        .unwrap()
    };

    let reader = std::thread::spawn(move || {
      let output = std::fs::read_to_string(&fifo_path).unwrap();
      assert_eq!(output, "hello\n");
    });

    writer.wait().unwrap();
    reader.join().unwrap();
  }
}
