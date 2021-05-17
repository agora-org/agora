use crate::{
  error::{Error, Result},
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
        .context(Error::filesystem_io(&file_path))?,
      path: file_path,
    })
  }
}

impl Stream for FileStream {
  type Item = Result<Bytes>;

  fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
    let data = &mut [MaybeUninit::uninit(); 8 * 1024];
    let mut buf = ReadBuf::uninit(data);

    let projected = self.project();

    let file = projected.file;
    let path = projected.path;

    let poll = file
      .poll_read(cx, &mut buf)
      .map(|result| result.context(Error::filesystem_io(path)))?;

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
}
