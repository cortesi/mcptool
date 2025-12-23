use std::{path::PathBuf, process::Stdio};

use chrono::Utc;
use tokio::{
    fs::{File, OpenOptions},
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    process::Command,
};

use crate::{Error, Result, target::Target};

/// Logs traffic data to the log file with timestamp and direction indicator.
async fn log_traffic(log_writer: &mut File, direction: &str, data: &[u8]) -> Result<()> {
    let timestamp = Utc::now().to_rfc3339();
    log_writer
        .write_all(format!("{timestamp}\n{direction}:\n").as_bytes())
        .await?;
    log_writer.write_all(data).await?;
    log_writer.write_all(b"\n").await?;
    log_writer.flush().await?;
    Ok(())
}

/// Forwards data from client to server and logs the traffic.
async fn handle_client_to_server<W>(
    data: &[u8],
    target: &mut W,
    log_writer: &mut File,
) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    target.write_all(data).await?;
    target.flush().await?;
    log_traffic(log_writer, "CLIENT->SERVER", data).await?;
    Ok(())
}

/// Forwards data from server to client and logs the traffic.
async fn handle_server_to_client<W>(
    data: &[u8],
    writer: &mut W,
    log_writer: &mut File,
) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    writer.write_all(data).await?;
    writer.flush().await?;
    log_traffic(log_writer, "SERVER->CLIENT", data).await?;
    Ok(())
}

pub async fn proxy_command(target: Target, log_file: PathBuf) -> Result<()> {
    let mut log_writer = Some(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .await?,
    );

    let stdin = io::stdin();
    let stdout = io::stdout();

    match target {
        Target::Tcp { host, port } => {
            let addr = format!("{host}:{port}");
            let target_stream = TcpStream::connect(&addr).await?;
            proxy_streams(stdin, stdout, target_stream, log_writer.as_mut().unwrap()).await?;
        }
        Target::Stdio { command, args } => {
            let mut cmd = Command::new(command);
            cmd.args(args);
            cmd.stdin(Stdio::piped());
            cmd.stdout(Stdio::piped());

            let mut child = cmd.spawn()?;
            let child_stdin = child.stdin.take().unwrap();
            let child_stdout = child.stdout.take().unwrap();

            proxy_process_streams(
                stdin,
                stdout,
                child_stdin,
                child_stdout,
                log_writer.as_mut().unwrap(),
            )
            .await?;
        }
        Target::Http { .. } | Target::Https { .. } => {
            return Err(Error::Other(
                "HTTP/HTTPS connections are not yet supported for proxy".to_string(),
            ));
        }
        Target::Auth { .. } => {
            return Err(Error::Other(
                "Auth targets should be resolved to actual targets before calling proxy_command"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

/// Proxies bidirectional traffic between reader/writer and a target stream.
async fn proxy_streams<R, W, T>(
    mut reader: R,
    mut writer: W,
    mut target: T,
    log_writer: &mut File,
) -> Result<()>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut buf1 = [0u8; 8192];
    let mut buf2 = [0u8; 8192];

    loop {
        tokio::select! {
            result = reader.read(&mut buf1) => {
                match result {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = &buf1[..n];
                        handle_client_to_server(data, &mut target, log_writer).await?;
                    }
                    Err(e) => return Err(Error::Io(e)),
                }
            }
            result = target.read(&mut buf2) => {
                match result {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = &buf2[..n];
                        handle_server_to_client(data, &mut writer, log_writer).await?;
                    }
                    Err(e) => return Err(Error::Io(e)),
                }
            }
        }
    }

    Ok(())
}

/// Proxies bidirectional traffic between reader/writer and a child process.
async fn proxy_process_streams<R, W, S, T>(
    mut reader: R,
    mut writer: W,
    mut target_stdin: S,
    mut target_stdout: T,
    log_writer: &mut File,
) -> Result<()>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
    S: AsyncWriteExt + Unpin,
    T: AsyncReadExt + Unpin,
{
    let mut buf1 = [0u8; 8192];
    let mut buf2 = [0u8; 8192];

    loop {
        tokio::select! {
            result = reader.read(&mut buf1) => {
                match result {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = &buf1[..n];
                        handle_client_to_server(data, &mut target_stdin, log_writer).await?;
                    }
                    Err(e) => return Err(Error::Io(e)),
                }
            }
            result = target_stdout.read(&mut buf2) => {
                match result {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = &buf2[..n];
                        handle_server_to_client(data, &mut writer, log_writer).await?;
                    }
                    Err(e) => return Err(Error::Io(e)),
                }
            }
        }
    }

    Ok(())
}
