use anyhow::Context;
use itertools::Itertools;
use nom::{
    branch::alt,
    bytes::streaming::{tag, take_until, take_until1},
    character::streaming::crlf,
    combinator::rest,
    sequence::{separated_pair, tuple},
    IResult, Parser,
};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
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
pub struct RequestLine {
    pub method: Method,
    pub path: String,
}

impl RequestLine {
    pub fn parse(input: &str) -> IResult<&str, RequestLine> {
        let space = &tag(" ");
        let until_space = take_until1(" ");

        let mut parser = tuple((Method::parse, space, until_space, space, rest))
            .map(|(method, _, path, _, _)| {
                let path = path.to_owned();

                RequestLine { method, path }
            })
            .map(|request_line| RequestLine {
                method: request_line.method,
                path: request_line.path,
            });

        parser.parse(input)
    }
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum Header {
    UserAgent,
}

impl Header {
    pub fn parse(input: &str) -> IResult<&str, Header> {
        let mut parser = alt((tag("User-Agent").map(|_| Header::UserAgent),));

        parser(input)
    }
}

fn parse_header_value(line: &str) -> IResult<&str, (Header, &str)> {
    let mut parser = separated_pair(Header::parse, tag(": "), rest);

    parser(line)
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
    fn test_parse_request_line() {
        let data = "GET / HTTP/1.1\r\n\r\n";
        let result = RequestLine::parse(data);

        let (rest, request) = result.expect("parse request");

        // Everything should be consumed
        assert_eq!(rest, "");

        assert!(matches!(request.method, Method::GET));
        assert_eq!(request.path, "/".to_owned());
    }
}

fn handle_socket(mut stream: TcpStream) -> anyhow::Result<()> {
    // TODO use Content-Length
    // let mut buffer = vec![0; 1024];
    // stream.read(&mut buffer).context("read stream")?;

    let mut reader = BufReader::new(&stream);
    let mut lines = reader.lines().flatten();

    let request_line = lines.next().context("read request line")?;

    let mut headers = HashMap::new();

    while let Some(header_line) = lines.next() {
        if header_line.is_empty() {
            break;
        }

        let Ok((_, (header, value))) = parse_header_value(&header_line) else {
            // Unknown header
            // TODO handle it?
            continue;
        };

        headers.insert(header, value.to_owned());
    }

    // TODO parse data

    // let a = reader.lines().flatten().take_while(|line| !line.is_empty());

    // let a = a.collect_vec();
    // dbg!(a);

    // let mut line = String::new();
    // let a = reader.read_line(&mut line);

    // a.lines()

    // let request_string = String::from_utf8(buffer).context("parse to utf8")?;

    let (_, request_line) = RequestLine::parse(&request_line)
        .map_err(|err| err.to_owned())
        .context("parse request")?;

    // let status_line = "HTTP/1.1 200 OK";
    // let headers = "";
    // let body = "";

    // let response = format!("{status_line}\r\n{headers}\r\n\r\n{body}");

    let response = if request_line.path.as_str() == "/" {
        let status_line = "HTTP/1.1 200 OK";
        let headers = "";
        let body = "";

        format!("{status_line}\r\n{headers}\r\n\r\n{body}")
    } else if request_line.path.as_str() == "/user-agent" {
        let user_agent = headers
            .get(&Header::UserAgent)
            .context("user-agent header not found")?;

        let status_line = "HTTP/1.1 200 OK";
        let headers = format!(
            "Content-Type: text/plain\r\nContent-Length: {}",
            user_agent.len()
        );
        let body = user_agent;

        format!("{status_line}\r\n{headers}\r\n\r\n{body}")
    } else if let Some(echo) = request_line.path.strip_prefix("/echo/") {
        let status_line = "HTTP/1.1 200 OK";
        let headers = format!("Content-Type: text/plain\r\nContent-Length: {}", echo.len());
        let body = echo;

        format!("{status_line}\r\n{headers}\r\n\r\n{body}")
    } else {
        let status_line = "HTTP/1.1 404 Not Found";
        let headers = "";
        let body = "";

        format!("{status_line}\r\n{headers}\r\n\r\n{body}")
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
