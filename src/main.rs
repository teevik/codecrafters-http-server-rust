use anyhow::Context;
use nom::{
    branch::alt,
    bytes::streaming::{tag, take_until, take_until1},
    character::streaming::crlf,
    combinator::rest,
    sequence::tuple,
    IResult, Parser,
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

#[derive(Debug)]
pub enum Method {
    GET,
    POST,
    PUT,
    DELETE,
}

impl Method {
    pub fn parse(data: &str) -> IResult<&str, Method> {
        let mut parser = alt((
            tag("GET").map(|_| Method::GET),
            tag("POST").map(|_| Method::POST),
            tag("PUT").map(|_| Method::PUT),
            tag("DELETE").map(|_| Method::DELETE),
        ));

        parser(data)
    }
}

#[derive(Debug)]
pub struct Request {
    pub method: Method,
    pub path: String,
}

impl Request {
    pub fn parse(input: &str) -> IResult<&str, Request> {
        let space = &tag(" ");
        let until_space = take_until1(" ");
        let until_crlf = &take_until("\r\n");

        struct RequestLine {
            method: Method,
            path: String,
        }

        let request_line = tuple((Method::parse, space, until_space, space, until_crlf)).map(
            |(method, _, path, _, _)| {
                let path = path.to_owned();

                RequestLine { method, path }
            },
        );

        let mut parser = tuple((request_line, crlf, until_crlf, crlf, rest)).map(
            |(request_line, _, _, _, _)| Request {
                method: request_line.method,
                path: request_line.path,
            },
        );

        parser.parse(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_method() {
        let data = "GET / HTTP/1.1\r\n";
        let result = Method::parse(data);

        assert!(matches!(result, Ok((_, Method::GET))));
    }

    #[test]
    fn test_parse_request() {
        let data = "GET / HTTP/1.1\r\n\r\n";
        let result = Request::parse(data);

        let (rest, request) = result.expect("parse request");

        // Everything should be consumed
        assert_eq!(rest, "");

        assert!(matches!(request.method, Method::GET));
        assert_eq!(request.path, "/".to_owned());
    }
}

fn handle_socket(mut stream: TcpStream) -> anyhow::Result<()> {
    // TODO use Content-Length
    let mut buffer = vec![0; 1024];
    stream.read(&mut buffer).context("read stream")?;

    let request_string = String::from_utf8(buffer).context("parse to utf8")?;

    let (_, request) = Request::parse(&request_string)
        .map_err(|err| err.to_owned())
        .context("parse request")?;

    let response = match request.path.as_str() {
        "/" => {
            let status_line = "HTTP/1.1 200 OK";
            let headers = "";
            let body = "";

            format!("{status_line}\r\n{headers}\r\n\r\n{body}")
        }

        _ => {
            let status_line = "HTTP/1.1 404 Not Found";
            let headers = "";
            let body = "";

            format!("{status_line}\r\n{headers}\r\n\r\n{body}")
        }
    };

    stream
        .write_all(response.as_bytes())
        .context("write response")?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        let stream = stream.context("accept connection")?;

        println!("accepted new connection");

        let result = handle_socket(stream);
        if let Err(error) = result {
            println!("error while handling socket: {:?}", error);
        }
    }

    Ok(())
}
