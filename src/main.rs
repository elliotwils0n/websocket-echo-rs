use base64ct::{Base64, Encoding};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const FIN_FLAG: u8 = 0b1000_0000;
const RSV1_FLAG: u8 = 0b0100_0000;
const RSV2_FLAG: u8 = 0b0010_0000;
const RSV3_FLAG: u8 = 0b0001_0000;
const MASK_FLAG: u8 = 0b1000_0000;

fn read_ws_message(mut stream: &TcpStream) -> Vec<u8> {
    let mut buf_reader = BufReader::new(&mut stream);
    let mut message = Vec::<u8>::new();

    loop {
        let mut buffer = [0u8; 2];
        buf_reader.read_exact(&mut buffer).unwrap();
        let fin: bool = buffer[0] & FIN_FLAG == FIN_FLAG;
        let _rsv1: bool = buffer[0] & RSV1_FLAG == RSV1_FLAG;
        let _rsv2: bool = buffer[0] & RSV2_FLAG == RSV2_FLAG;
        let _rsv3: bool = buffer[0] & RSV3_FLAG == RSV3_FLAG;
        let _opcode: u8 = 0b0000_1111 & buffer[0];
        let mask: bool = buffer[1] & MASK_FLAG == MASK_FLAG;
        let initial_len: u8 = 0b0111_1111 & buffer[1];

        let payload_len: u64;
        let masking_key: Option<Vec<u8>>;
        let payload_data: Vec<u8>;

        if initial_len < 126 {
            payload_len = initial_len as u64;
        } else if initial_len == 126 {
            let mut buffer = [0u8; 2];
            buf_reader.read_exact(&mut buffer).unwrap();
            // todo: maybe some network to hardware translation
            let extended_payload_len: u16 = u16::from_be_bytes(buffer);
            payload_len = extended_payload_len as u64;
        } else if initial_len == 127 {
            let mut buffer = [0u8; 8];
            buf_reader.read_exact(&mut buffer).unwrap();
            // todo: maybe some network to hardware translation
            let extended_payload_len: u64 = u64::from_be_bytes(buffer);
            payload_len = extended_payload_len;
        } else {
            panic!("Unexpected payload_len value");
        }

        if mask {
            let mut buffer = [0u8; 4];
            buf_reader.read_exact(&mut buffer).unwrap();
            masking_key = Some(buffer.to_vec());
        } else {
            masking_key = None;
        }

        let mut buffer = Vec::<u8>::with_capacity(payload_len as usize);
        (0..payload_len).into_iter().for_each(|_| buffer.push(0u8));
        buf_reader.read_exact(&mut buffer).unwrap();

        payload_data = match masking_key {
            Some(mkey) => {
                let mut unmasked_payload = Vec::<u8>::with_capacity(payload_len as usize);
                (0..payload_len).into_iter().for_each(|i| {
                    unmasked_payload.push(buffer[i as usize] ^ mkey[i as usize % 4]);
                });
                unmasked_payload
            }
            None => buffer,
        };

        message.extend(payload_data);
        if fin {
            break;
        }
    }
    message
}

fn write_ws_message(mut stream: &TcpStream, message: String) {
    let mut response = Vec::<u8>::new();
    // fin(true), rcv1(false), rcv2(false), rcv3(false), opcode(0001 -> text)
    response.push(0b1000_0001);

    if message.len() < 126 {
        // message len with mask off
        response.push(message.len() as u8 & (!MASK_FLAG));
    } else if message.len() < u16::MAX as usize {
        // magic length for next 2 bytes as len with mask off
        response.push(126u8 & (!MASK_FLAG));
        let length_bytes = (message.len() as u16).to_le_bytes();
        response.extend(length_bytes);
    } else if message.len() < u64::MAX as usize {
        // magic length for next 8 bytes as len with mask off
        response.push(127u8 & (!MASK_FLAG));
        let length_bytes = (message.len() as u64).to_le_bytes();
        response.extend(length_bytes);
    } else {
        unimplemented!("Multiple frames not supported");
    }

    response.extend(message.as_bytes());

    stream.write_all(&response).unwrap();
    stream.flush().unwrap();
}

fn read_lines(mut stream: &TcpStream) -> Vec<String> {
    let buf_reader = BufReader::new(&mut stream);
    buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect()
}

fn generate_response_accept_header(client_websocket_key: &str) -> String {
    let websocket_accept = format!("{client_websocket_key}{GUID}");

    let mut hasher = Sha1::new();
    hasher.update(websocket_accept.as_bytes());
    let websocket_accept_sha1 = hasher.finalize();

    Base64::encode_string(&websocket_accept_sha1)
}

fn websocket_accept_bytes(request: Vec<String>) -> Vec<u8> {
    let client_websocket_key = &request
        .iter()
        .filter(|it| it.starts_with("Sec-WebSocket-Key:"))
        .map(|it| it.split(":").collect::<Vec<_>>()[1].trim())
        .collect::<Vec<_>>()[0];
    let websocket_accept = generate_response_accept_header(client_websocket_key);

    let metadata = "HTTP/1.1 101 Switching Protocols";
    let headers = HashMap::<&str, &str>::from([
        ("Upgrade", "websocket"),
        ("Connection", "Upgrade"),
        ("Sec-WebSocket-Accept", &websocket_accept),
    ]);

    let mut response = format!("{metadata}\r\n");
    headers
        .iter()
        .for_each(|(k, v)| response.push_str(format!("{k}: {v}\r\n").as_str()));
    response.push_str("\r\n");
    response.as_bytes().to_vec()
}

fn handle_connection(mut stream: TcpStream) {
    let request = read_lines(&stream);
    let accept_response = websocket_accept_bytes(request);
    stream.write_all(&accept_response).unwrap();
    stream.flush().unwrap();

    loop {
        let message = String::from_utf8(read_ws_message(&stream)).unwrap();
        println!("Received message: {message}");
        if message == "close" {
            write_ws_message(&stream, "Closing connection. Good bye ;>".to_owned());
            break;
        }
        write_ws_message(&stream, format!("Echo: {}", message));
    }
}

fn main() {
    let address = "127.0.0.1:8010";
    let listener = TcpListener::bind(&address).unwrap();
    println!("Listening at address {address}...");
    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            thread::spawn(move || {
                handle_connection(stream);
            });
        }
    }
}
