use std::{env, io::Write, net::TcpListener};

fn main() {
    let auth_port = env::var("AUTH_PORT").unwrap_or("10000".to_string());
    let auth_server = env::var("AUTH_SERVER").unwrap_or("127.0.0.1".to_string());
    let listen = env::var("LISTEN_ON").unwrap_or("127.0.0.1:3330".to_string());
    println!("Target SMTP server: {}:{}", auth_server, auth_port);
    println!("Listening to: {}", listen);
    let listener = TcpListener::bind(listen).expect("Cannot listen");

    let mut buf = String::new();
    buf.push_str("HTTP/1.0 200 OK\r\n");
    buf.push_str("Auth-Status: OK\r\n");
    buf.push_str(&format!("Auth-Server: {}\r\n", auth_server));
    buf.push_str(&format!("Auth-Port: {}\r\n", auth_port));
    buf.push_str("Content-Length: 0\r\n\r\n");

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                println!("New request from {:?}", s.peer_addr());
                s.write_all(buf.as_bytes()).unwrap();
            }
            Err(e) => {
                println!("Bad stream: {}", e.to_string());
            }
        }
    }
}
