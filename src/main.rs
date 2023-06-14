mod safe_read_write;

use std::{
    collections::HashMap,
    env,
    fs::{File, OpenOptions},
    io::{stdout, Read, Seek, SeekFrom, Write},
    net::*,
    str::FromStr,
    thread,
    time::{Duration, SystemTime},
};

use time::{Date, PrimitiveDateTime, Time};

use crate::safe_read_write::{SafeReadWrite, Wrap};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).unwrap_or(&"version".to_owned()).as_str() {
        "helper" => helper(&args),
        "sender" => sender(&args, |_| {}),
        "receiver" => receiver(&args, |_| {}),
        "version" => println!("QUAD version: {}", env!("CARGO_PKG_VERSION")),
        _ => (),
    }
}

pub fn helper(args: &Vec<String>) {
    let bind_addr = (
        "0.0.0.0",
        u16::from_str_radix(args[2].as_str(), 10).expect("invalid port: must be integer"),
    );
    let mut map: HashMap<[u8; 200], SocketAddr> = HashMap::new();
    let listener = UdpSocket::bind(&bind_addr).expect("unable to create socket");
    let mut buf = [0 as u8; 200];
    let mut last_log_time = unix_millis();
    let mut amount_since_log = 0;
    let mut helper_log = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open("qft_helper_log.txt")
        .expect("unable to create helper log");
    loop {
        let (l, addr) = listener.recv_from(&mut buf).expect("read error");
        if l != 200 {
            continue;
        }
        if map.contains_key(&buf) {
            let other = map.get(&buf).unwrap();
            // we got a connection
            let mut bytes: &[u8] = addr.to_string().bytes().collect::<Vec<u8>>().leak();
            let mut addr_buf = [0 as u8; 200];
            for i in 0..bytes.len().min(200) {
                addr_buf[i] = bytes[i];
            }
            bytes = other.to_string().bytes().collect::<Vec<u8>>().leak();
            let mut other_buf = [0 as u8; 200];
            for i in 0..bytes.len().min(200) {
                other_buf[i] = bytes[i];
            }
            if listener.send_to(&addr_buf, other).is_ok()
                && listener.send_to(&other_buf, addr).is_ok()
            {
                // success!
                println!("Helped {} and {}! :D", addr, other);
                amount_since_log += 1;
                if unix_millis() - last_log_time > 10000 {
                    let d = PrimitiveDateTime::new(
                        Date::from_calendar_date(1970, time::Month::January, 1).unwrap(),
                        Time::MIDNIGHT,
                    ) + Duration::from_millis(unix_millis());
                    helper_log
                        .write(
                            format!(
                                "{} | {} {}>\n",
                                d,
                                amount_since_log,
                                amount_since_log * Wrap("=")
                            )
                            .as_bytes(),
                        )
                        .expect("error writing to log");
                    helper_log.flush().expect("error writing to log");
                    last_log_time = unix_millis();
                    amount_since_log = 0;
                }
            }
            map.remove(&buf);
        } else {
            map.insert(buf, addr);
        }
    }
}

pub fn sender<F: Fn(f32)>(args: &[String], on_progress: F) {
    // Establish connection
    let connection = holepunch(args);

    // Parse bitrate argument or set to default of 256
    let bitrate = args
        .get(5)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(256);

    // Parse begin argument or set to default of 0
    let start_position = args.get(6).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);

    // Initialize buffer with size of bitrate
    let mut buffer: Vec<u8> = vec![0; bitrate as usize];

    // Open file for reading
    let file_path = &args[4];
    let mut file = File::open(file_path).expect("Unable to open file for reading");

    // Seek to start position if not zero
    if start_position != 0 {
        println!("Skipping to {}...", start_position);
        file.seek(SeekFrom::Start(start_position))
            .expect("Unable to seek to start position");
        println!("Done.");
    }

    // Initialize safe reader/writer
    let mut safe_rw = SafeReadWrite::new(connection);

    let mut total_sent: u64 = 0;
    let mut last_update_time = unix_millis();

    // Send file length to receiver
    let file_length = file.metadata().expect("Unable to read file metadata").len();
    safe_rw
        .write_safe(&file_length.to_be_bytes())
        .expect("Unable to send file length");
    println!("File length: {}", file_length);

    loop {
        // Read data from file
        let read_size = file.read(&mut buffer).expect("Error reading file");

        // If end of file is reached and not in stream mode, end the transfer
        if read_size == 0 && env::var("QFT_STREAM").is_err() {
            println!("\nTransfer complete. Thank you!");
            safe_rw.end();
            return;
        }

        // Send data to receiver
        safe_rw
            .write_safe(&buffer[..read_size])
            .expect("Error sending data");

        total_sent += read_size as u64;

        // Display progress
        if (total_sent % (bitrate * 20)) < bitrate {
            print!("\r\x1b[KSent {} bytes", total_sent);
            stdout().flush().unwrap();
        }

        // Update progress
        if unix_millis() - last_update_time > 100 {
            on_progress((total_sent + start_position) as f32 / file_length as f32);
            last_update_time = unix_millis();
        }
    }
}

pub fn receiver<F: Fn(f32)>(args: &[String], on_progress: F) {
    // Establish connection
    let connection = holepunch(args);

    // Parse bitrate argument or set to default of 256
    let bitrate = args
        .get(5)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(256);

    // Parse begin argument or set to default of 0
    let start_position = args.get(6).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);

    // Initialize buffer with size of bitrate
    let buffer: Vec<u8> = vec![0; bitrate as usize];

    // Open file for writing
    let file_path = &args[4];
    let mut file = OpenOptions::new()
        .truncate(false)
        .write(true)
        .create(true)
        .open(file_path)
        .expect("Unable to open file for writing");

    // Seek to start position if not zero
    if start_position != 0 {
        println!("Skipping to {}...", start_position);
        file.seek(SeekFrom::Start(start_position))
            .expect("Unable to seek to start position");
        println!("Done.");
    }

    // Initialize safe reader/writer
    let mut safe_rw = SafeReadWrite::new(connection);

    let mut total_received: u64 = 0;
    let mut last_update_time = unix_millis();

    // Read file length from sender
    let len_bytes = [0_u8; 8];
    let len_arr = safe_rw
        .read_safe(&len_bytes)
        .expect("Unable to read file length from sender")
        .0;
    let file_length = u64::from_be_bytes([
        len_arr[0], len_arr[1], len_arr[2], len_arr[3], len_arr[4], len_arr[5], len_arr[6],
        len_arr[7],
    ]);
    file.set_len(file_length)
        .expect("Unable to set len for file.");

    println!("File length: {}", file_length);

    loop {
        // Read data from sender
        let (received_buf, received_amount) =
            safe_rw.read_safe(&buffer).expect("Error reading data");
        let data_chunk = &received_buf[..received_amount];

        if received_amount == 0 {
            println!("\nTransfer complete. Thank you!");
            return;
        }

        // Write data to file
        file.write_all(data_chunk)
            .expect("Error writing data to file");
        file.flush().expect("Error flushing file");

        total_received += received_amount as u64;

        // Display progress
        if (total_received % (bitrate * 20)) < bitrate {
            print!("\r\x1b[KReceived {} bytes;", total_received);
            stdout().flush().unwrap();
        }

        // Update progress
        if unix_millis() - last_update_time > 100 {
            on_progress((total_received + start_position) as f32 / file_length as f32);
            last_update_time = unix_millis();
        }
    }
}

fn holepunch(args: &[String]) -> UdpSocket {
    // Initialize socket
    let bind_addr = (Ipv4Addr::from(0_u32), 0);
    let holepunch = UdpSocket::bind(bind_addr).expect("Unable to create socket");

    // Connect to helper
    let helper_address = &args[2];
    holepunch
        .connect(helper_address)
        .expect("Unable to connect to helper");

    // Send data to helper
    let data = args[3].as_bytes();
    let mut buf = [0_u8; 200];
    buf[..data.len().min(200)].copy_from_slice(&data[..data.len().min(200)]);
    holepunch.send(&buf).expect("Unable to talk to helper");

    // Receive data from helper
    holepunch
        .recv(&mut buf)
        .expect("Unable to receive from helper");

    // Process helper data and reconnect to partner
    let mut s = Vec::from(buf);
    s.retain(|e| *e != 0);
    let partner_address = String::from_utf8_lossy(s.as_slice()).to_string();
    println!(
        "Holepunching {} (partner) and :{} (you).",
        partner_address,
        holepunch.local_addr().unwrap().port()
    );
    holepunch
        .connect(SocketAddrV4::from_str(partner_address.as_str()).unwrap())
        .expect("Connection to partner failed");

    // Set timeouts
    holepunch
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    holepunch
        .set_write_timeout(Some(Duration::from_secs(1)))
        .unwrap();

    // connect
    println!("Connecting...");
    thread::sleep(Duration::from_millis(500 - (unix_millis() % 500)));
    for _ in 0..40 {
        let m = unix_millis();
        let _ = holepunch.send(&[0]);
        thread::sleep(Duration::from_millis((50 - (unix_millis() - m)).max(0)));
    }

    // receive and send data
    let mut result = Ok(1);
    while result.is_ok() && result.unwrap() == 1 {
        result = holepunch.recv(&mut [0, 0]);
    }
    holepunch.send(&[0, 0]).expect("Connection failed");
    holepunch.send(&[0, 0]).expect("Connection failed");

    // confirm connection
    result = Ok(1);
    while result.is_ok() && result.unwrap() != 2 {
        result = holepunch.recv(&mut [0, 0]);
    }
    result = Ok(1);
    while result.is_ok() && result.unwrap() == 2 {
        result = holepunch.recv(&mut [0, 0]);
    }

    println!("Holepunch and connection successful.");
    holepunch
}

pub fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
