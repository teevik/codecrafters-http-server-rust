use anyhow::Context;
use nom::{
    branch::alt,
    bytes::streaming::{tag, take_until1},
    combinator::rest,
    sequence::{separated_pair, tuple},
    IResult, Parser,
};
use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
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
    ContentType,
    ContentLength,
}

impl Header {
    pub fn parse(input: &str) -> IResult<&str, Header> {
        let mut parser = alt((
            tag("User-Agent").map(|_| Header::UserAgent),
            tag("Content-Type").map(|_| Header::ContentType),
            tag("Content-Length").map(|_| Header::ContentLength),
        ));

        parser(input)
    }
}

impl Display for Header {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Header::UserAgent => write!(f, "User-Agent"),
            Header::ContentType => write!(f, "Content-Type"),
            Header::ContentLength => write!(f, "Content-Length"),
        }
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

pub enum Status {
    Ok,
    NotFound,
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Status::Ok => write!(f, "200 OK"),
            Status::NotFound => write!(f, "404 Not Found"),
        }
    }
}

struct Response {
    status: Status,
    headers: HashMap<Header, String>,
    body: String,
}

impl Display for Response {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "HTTP/1.1 {}\r\n", self.status)?;

        for (header, value) in &self.headers {
            write!(f, "{}: {}\r\n", header, value)?;
        }

        write!(f, "\r\n{}", self.body)
    }
}

async fn handle_socket(mut stream: TcpStream) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.split();

    let reader = BufReader::new(reader);
    let mut lines = reader.lines();

    let request_line = lines
        .next_line()
        .await
        .context("read request line")?
        .context("no request line")?;

    let mut headers = HashMap::new();

    while let Some(header_line) = lines.next_line().await.context("read header")? {
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

    let (_, request_line) = RequestLine::parse(&request_line)
        .map_err(|err| err.to_owned())
        .context("parse request")?;

    let response = if request_line.path.as_str() == "/" {
        Response {
            status: Status::Ok,
            headers: HashMap::new(),
            body: String::new(),
        }
    } else if request_line.path.as_str() == "/user-agent" {
        let user_agent = headers
            .get(&Header::UserAgent)
            .context("user-agent header not found")?;

        let headers = HashMap::from_iter([
            (Header::ContentType, "text/plain".to_owned()),
            (Header::ContentLength, user_agent.len().to_string()),
        ]);

        let body = user_agent.clone();

        Response {
            status: Status::Ok,
            headers,
            body,
        }
    } else if let Some(echo) = request_line.path.strip_prefix("/echo/") {
        let headers = HashMap::from_iter([
            (Header::ContentType, "text/plain".to_owned()),
            (Header::ContentLength, echo.len().to_string()),
        ]);

        let body = echo.to_owned();

        Response {
            status: Status::Ok,
            headers,
            body,
        }
    } else {
        Response {
            status: Status::NotFound,
            headers: HashMap::new(),
            body: String::new(),
        }
    };

    writer
        .write_all(response.to_string().as_bytes())
        .await
        .context("write response")?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4221")
        .await
        .context("bind socket")?;

    loop {
        let (socket, _) = listener.accept().await.context("accept listener")?;

        println!("accepted new connection");

        tokio::spawn(handle_socket(socket));
    }

    Ok(())
}
