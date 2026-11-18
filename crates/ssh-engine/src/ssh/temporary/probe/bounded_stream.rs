//! russh-sftp 2.3's raw client does not cap received packet lengths. Validate
//! each length prefix before exposing it to the client's allocating parser.
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

const MAX_PACKET: usize = 4096;

pub(in crate::ssh) struct BoundedSftpStream<S> {
    inner: S,
    header: [u8; 4],
    filled: usize,
    delivered: usize,
    remaining: usize,
    invalid: bool,
    max_packet: usize,
}

impl<S> BoundedSftpStream<S> {
    pub(in crate::ssh) fn new(inner: S) -> Self {
        Self::new_with_limit(inner, MAX_PACKET)
    }
    pub(in crate::ssh) fn new_with_limit(inner: S, max_packet: usize) -> Self {
        Self {
            inner,
            header: [0; 4],
            filled: 0,
            delivered: 0,
            remaining: 0,
            invalid: false,
            max_packet,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for BoundedSftpStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if output.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        if this.invalid {
            // The library retries non-EOF reader errors. End the rejected
            // stream after one error instead of allowing a busy retry loop.
            return Poll::Ready(Ok(()));
        }
        while this.filled < 4 {
            let mut header = ReadBuf::new(&mut this.header[this.filled..]);
            match Pin::new(&mut this.inner).poll_read(cx, &mut header) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(error)) => {
                    this.invalid = true;
                    return Poll::Ready(Err(error));
                }
                Poll::Ready(Ok(())) => {
                    let read = header.filled().len();
                    if read == 0 {
                        return if this.filled == 0 {
                            Poll::Ready(Ok(()))
                        } else {
                            Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()))
                        };
                    }
                    this.filled += read;
                }
            }
            if this.filled == 4 {
                this.remaining = u32::from_be_bytes(this.header) as usize;
                if this.remaining == 0 || this.remaining > this.max_packet {
                    this.invalid = true;
                    return Poll::Ready(Err(io::ErrorKind::InvalidData.into()));
                }
            }
        }
        if this.delivered < 4 {
            let amount = (4 - this.delivered).min(output.remaining());
            output.put_slice(&this.header[this.delivered..this.delivered + amount]);
            this.delivered += amount;
            return Poll::Ready(Ok(()));
        }
        let mut storage = [0; MAX_PACKET];
        let amount = this.remaining.min(output.remaining()).min(MAX_PACKET);
        let mut body = ReadBuf::new(&mut storage[..amount]);
        match Pin::new(&mut this.inner).poll_read(cx, &mut body) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => {
                this.invalid = true;
                Poll::Ready(Err(error))
            }
            Poll::Ready(Ok(())) => {
                let read = body.filled().len();
                if read == 0 {
                    return Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()));
                }
                output.put_slice(body.filled());
                this.remaining -= read;
                if this.remaining == 0 {
                    this.filled = 0;
                    this.delivered = 0;
                }
                Poll::Ready(Ok(()))
            }
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for BoundedSftpStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, bytes)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn rejects_oversize_before_exposing_header_to_allocating_parser() {
        for length in [0_u32, 4097, u32::MAX] {
            let bytes = length.to_be_bytes();
            let mut stream = BoundedSftpStream::new(bytes.as_slice());
            let mut output = [0; 4];
            assert_eq!(
                stream.read_exact(&mut output).await.unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
            assert_eq!(output, [0; 4]);
            assert_eq!(stream.read(&mut output).await.unwrap(), 0);
        }
    }

    #[tokio::test]
    async fn fragmented_frames_and_writes_survive_small_buffers() {
        let (mut server, client) = tokio::io::duplex(2);
        let expected = [0, 0, 0, 3, 1, 2, 3, 0, 0, 0, 1, 4];
        let task = tokio::spawn(async move {
            for byte in expected {
                server.write_all(&[byte]).await.unwrap();
            }
            let mut request = [0; 3];
            server.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"req");
        });
        let mut stream = BoundedSftpStream::new(client);
        let mut received = [0; 12];
        for byte in &mut received {
            *byte = stream.read_u8().await.unwrap();
        }
        assert_eq!(received, expected);
        stream.write_all(b"req").await.unwrap();
        task.await.unwrap();
    }

    #[tokio::test]
    async fn truncated_header_and_body_are_errors() {
        for bytes in [vec![0, 0], vec![0, 0, 0, 2, 1]] {
            let mut stream = BoundedSftpStream::new(bytes.as_slice());
            assert_eq!(
                stream
                    .read_to_end(&mut Vec::new())
                    .await
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::UnexpectedEof
            );
        }
    }
}
