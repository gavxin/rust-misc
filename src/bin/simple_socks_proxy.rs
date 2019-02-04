use std::io;
use std::io::{Read, Write, Error, ErrorKind};
use std::net::{TcpListener, TcpStream, Shutdown, SocketAddr, SocketAddrV4, Ipv4Addr, Ipv6Addr};
use std::thread;
use std::time::Duration;

fn handle_method_selection(stream: &mut TcpStream) -> io::Result<()> {
    let mut ver_nmethod = [0u8; 2];
    stream.read_exact(&mut ver_nmethod)?;

    if ver_nmethod[0] != 4u8 && ver_nmethod[0] != 5u8 {
        return Err(Error::new(ErrorKind::Other, "Version mismatch!"));
    }

    println!("Version:{} nMethods:{}", ver_nmethod[0], ver_nmethod[1]);

    let mut methods = vec![0; ver_nmethod[1] as usize];
    stream.read_exact(methods.as_mut_slice())?;

    let mut found = false;
    for method in methods {
        if method == 0u8 {
            found = true;
        }
        println!("Method: {}", method);
    }
    
    if !found {
        return Err(Error::new(ErrorKind::Other, "Does not support other method. ONLY No Authentication method support"));
    }

    let response = [5u8, 0u8];
    stream.write(&response)?;

    Ok(())
}

fn handle_request(stream: &mut TcpStream) -> io::Result<()> {
    
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf)?;

    assert!(buf[0] == 5u8);
    assert!(buf[2] == 0u8);

    let cmd = buf[1];
    let atyp = buf[3];

    if cmd != 1u8 /* CONNECT */ {
        println!("CMD is {}. NOT SUPPORTED!", cmd);
        return Err(Error::new(ErrorKind::Other, "CMD not supported!"));
    }

    let mut s : String;
    match atyp {
        1u8 /* IP v4 */ => {
            let mut tmp = [0u8; 6];
            stream.read_exact(&mut tmp)?;
            let ip = Ipv4Addr::new(tmp[0], tmp[1], tmp[2], tmp[3]);
            let mut port = ((tmp[4] as u16) << 8) + tmp[5] as u16;
            s = format!("{}:{}", ip, port);
        }
        3u8 /* DOMAIN */ => {
            let mut tmp = [0u8; 1];
            stream.read_exact(&mut tmp)?;
            let mut domain_buf = vec![0u8; tmp[0] as usize];
            stream.read_exact(&mut domain_buf)?;
            let mut port_buf = [0u8; 2];
            stream.read_exact(&mut port_buf)?;
            let mut port = ((port_buf[0] as u16) << 8) + port_buf[1] as u16;
            s = format!("{}:{}", String::from_utf8(domain_buf).unwrap(), port); 
        }
        _ => {
            return Err(Error::new(ErrorKind::Other, "Not support"));
        }
    }

    println!("Try connect to {}", s);
    let r = TcpStream::connect(s);

    if let Err(e) = r {
        println!("Connect failed {:?}", e);
        let reply_buf = [
            5u8,
            1u8,
            0u8,
            1u8,
            0u8, 0u8, 0u8, 0u8,
            0u8, 0u8
        ];
        stream.write(&reply_buf)?;
        stream.shutdown(Shutdown::Both)?;
        return Err(e);
    }

    let mut upstream = r.unwrap();
    println!("Connect succeed");

    let reply_buf = [
        5u8,
        0u8,
        0u8,
        1u8,
        0u8, 0u8, 0u8, 0u8,
        0u8, 0u8
    ];

    stream.write(&reply_buf)?;

    // stream.set_read_timeout(Some(Duration::from_secs(30)));
    // upstream.set_read_timeout(Some(Duration::from_secs(30)));
    let mut downstream_clone = stream.try_clone().expect("Clone failed.");
    let mut upstream_clone = upstream.try_clone().expect("Clone failed.");
    // stream.set_nonblocking(true);
    // upstream.set_nonblocking(true);

    thread::spawn(move || {
        let mut buff = [0u8; 1024 * 1024];

        loop {
            match upstream_clone.read(&mut buff) {
                Ok(n) => {
                    if n == 0 {
                        println!("Readed 0 bytes!");
                        // downstream_clone.shutdown(Shutdown::Both);
                        // upstream_clone.shutdown(Shutdown::Both);
                        return; // Err(Error::new(ErrorKind::Other, "Readed 0 bytes! may disconnected!"));
                    }

                    println!("Read from server bytes:{}", n);
                    downstream_clone.write_all(&buff[..n]).unwrap();
                },
                Err(e) => {
                    if (e.kind() == ErrorKind::WouldBlock) {
                        // Not error
                        println!("WOULDBLOCK!!!!!!!!!!!!");
                    } else {
                        // downstream_clone.shutdown(Shutdown::Both);
                        // upstream_clone.shutdown(Shutdown::Both);
                        return; // Err(e);
                    }
                }
            }
        }
    });
    
    let mut buff = [0u8; 1024 * 512];
    loop {
        match stream.read(&mut buff) {
            Ok(n) => {
                if n == 0 {
                    println!("Readed 0 bytes!");
                    // stream.shutdown(Shutdown::Both);
                    // upstream.shutdown(Shutdown::Both);
                    return Err(Error::new(ErrorKind::Other, "Readed 0 bytes! may disconnected!"));
                }

                println!("Read from client bytes:{}", n);
                upstream.write_all(&buff[..n])?;
            },
            Err(e) => {
                if e.kind() == ErrorKind::WouldBlock {
                    // Not error
                    println!("WOULDBLOCK!!!!!!!!!!!!");
                } else {
                    // stream.shutdown(Shutdown::Both);
                    // upstream.shutdown(Shutdown::Both);
                    return Err(e);
                }
            }
        }
    }

    // stream.shutdown(Shutdown::Both);
    // upstream.shutdown(Shutdown::Both);
    Ok(())
}

fn handle_client(mut stream: TcpStream) -> io::Result<()> {
    println!("Client connected.");
    handle_method_selection(&mut stream)?;
    handle_request(&mut stream)?;
    Ok(())
}

fn main() -> io::Result<()> {
    let s = "127.0.0.1:1080";
    let listener = TcpListener::bind(s)?;
    println!("Listening on {}", s);
    
    for stream in listener.incoming() {
        let s = stream.unwrap();
        thread::spawn(move || {
            let ret = handle_client(s);
            dbg!(ret);
        });
    }

    Ok(())
}
