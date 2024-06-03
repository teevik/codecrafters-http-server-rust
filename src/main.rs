use std::{io::Write, net::TcpListener};

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        let mut stream = stream.context("accept connection")?;

        println!("accepted new connection");

        let status_line = "HTTP/1.1 200 OK";
        let headers = "";
        let body = "";

        let response = format!("{status_line}\r\n{headers}\r\n\r\n{body}");

        stream
            .write_all(response.as_bytes())
            .context("write response")?;
    }

    Ok(())
}
