#[derive(Ord, Eq, PartialOrd, PartialEq)]
enum SafeReadWritePacket {
    Write,
    Ack,
    ResendRequest,
    End,
}
use std::{collections::HashMap, env, io::Error, net::UdpSocket, time::Duration};

use SafeReadWritePacket::*;

use crate::unix_millis;

pub struct SafeReadWrite {
    socket: UdpSocket,
    last_transmitted: HashMap<u16, Vec<u8>>,
    packet_count_out: u64,
    packet_count_in: u64,
}

impl SafeReadWrite {
    pub fn new(socket: UdpSocket) -> SafeReadWrite {
        SafeReadWrite {
            socket,
            last_transmitted: HashMap::new(),
            packet_count_in: 0,
            packet_count_out: 0,
        }
    }

    pub fn write_safe(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.write_flush_safe(buf, false)
    }

    pub fn write_flush_safe(&mut self, buf: &[u8], flush: bool) -> Result<(), Error> {
        self.internal_write_safe(buf, Write, flush, false)
    }

    pub fn read_safe(&mut self, buf: &[u8]) -> Result<(Vec<u8>, usize), Error> {
        if buf.len() > 0xfffc {
            panic!(
                "attempted to receive too large data packet with SafeReadWrite ({} > 0xfffc)",
                buf.len()
            );
        }

        let mut mbuf = Vec::from(buf);
        mbuf.insert(0, 0);
        mbuf.insert(0, 0);
        mbuf.insert(0, 0);
        let buf: &mut [u8] = mbuf.as_mut();

        let mut r = (vec![], 0);

        let mut try_again = true;
        let mut is_catching_up = false;
        while try_again {
            if let Ok(x) = self.socket.recv(buf) {
                if x < 3 {
                    continue;
                }
                let id = u16::from_be_bytes([buf[0], buf[1]]);
                if id <= self.packet_count_in as u16 {
                    self.socket
                        .send(&[buf[0], buf[1], Ack as u8])
                        .expect("send error");
                }
                if id == self.packet_count_in as u16 {
                    if id == 0xffff {
                        println!("\nPacket ID wrap successful.");
                    }
                    try_again = false;
                    self.packet_count_in += 1;
                    r.1 = x - 3;
                } else if id > self.packet_count_in as u16
                    && (id - self.packet_count_in as u16) < 0xC000
                {
                    if !is_catching_up && env::var("QUAD_HIDE_DROPS").is_err() {
                        println!(
                            "\r\x1b[KA packet dropped: {} (got) is newer than {} (expected)",
                            &id,
                            &(self.packet_count_in as u16)
                        );
                    }
                    is_catching_up = true;
                    // ask to resend, then do nothing
                    let id = (self.packet_count_in as u16).to_be_bytes();
                    self.socket
                        .send(&[id[0], id[1], ResendRequest as u8])
                        .expect("send error");
                }
                if buf[2] == End as u8 {
                    return Ok((vec![], 0));
                }
            }
        }
        mbuf.remove(0);
        mbuf.remove(0);
        mbuf.remove(0);
        r.0 = mbuf;
        Ok(r)
    }

    pub fn end(mut self) -> UdpSocket {
        let _ = self.internal_write_safe(&[], End, true, true);

        self.socket
    }

    fn internal_write_safe(
        &mut self,
        buf: &[u8],
        packet: SafeReadWritePacket,
        flush: bool,
        exit_on_lost: bool,
    ) -> Result<(), Error> {
        if buf.len() > 0xfffc {
            panic!(
                "too large data packet sent over SafeReadWrite ({} > 0xfffc)",
                buf.len()
            );
        }

        let id = (self.packet_count_out as u16).to_be_bytes();
        let idn = self.packet_count_out as u16;
        self.packet_count_out += 1;

        let mut vbuf = Vec::from(buf);
        vbuf.insert(0, packet as u8);
        vbuf.insert(0, id[1]);
        vbuf.insert(0, id[0]); // this is now the first byte
        let buf = vbuf.as_slice();

        loop {
            match self.socket.send(buf) {
                Ok(x) => {
                    if x != buf.len() {
                        continue;
                    }
                }
                Err(_) => {
                    continue;
                }
            }
            self.last_transmitted.insert(idn, vbuf);
            break;
        }
        let mut buf = [0, 0, 0];
        let mut wait = idn == 0xffff || flush;
        if self.last_transmitted.len() < 256 {
            self.socket
                .set_read_timeout(Some(Duration::from_millis(1)))
                .unwrap();
        } else {
            wait = true;
        }
        let mut start = unix_millis();
        if idn == 0xffff {
            print!("\nPacket ID needs to wrap. Waiting for partner to catch up...")
        }
        let mut is_catching_up = false;
        loop {
            match self.socket.recv(&mut buf).ok() {
                Some(x) => {
                    if x != 3 {
                        continue;
                    }
                    if buf[2] == Ack as u8 {
                        let n = u16::from_be_bytes([buf[0], buf[1]]);
                        self.last_transmitted.remove(&n);
                        if n == idn {
                            if idn == 0xffff {
                                println!("\r\x1b[KPacket ID wrap successful.");
                            }
                            wait = false;
                            self.last_transmitted.clear(); // if the latest packet is ACK'd, all
                                                           // previous ones must be as well.
                        }
                    }
                    if buf[2] == ResendRequest as u8 {
                        let mut n = u16::from_be_bytes([buf[0], buf[1]]);
                        if !is_catching_up && env::var("QUAD_HIDE_DROPS").is_err() {
                            println!("\r\x1b[KA packet dropped: {}", &n);
                        }
                        wait = true;
                        is_catching_up = true;
                        while n <= idn && !(idn == 0xffff && n == 0) {
                            let buf = self.last_transmitted.get(&n);
                            if let Some(buf) = buf {
                                loop {
                                    // resend until success
                                    match self.socket.send(buf.as_slice()) {
                                        Ok(x) => {
                                            if x != buf.len() {
                                                continue;
                                            }
                                        }
                                        Err(_) => {
                                            continue;
                                        }
                                    };
                                    break;
                                }
                            } else {
                                break;
                            }
                            // do NOT remove from last_transmitted yet, wait for Ack to do that.
                            n += 1;
                        }
                    }
                }
                None => {
                    if unix_millis() - start > 5000 && exit_on_lost {
                        break;
                    }
                    if unix_millis() - start > 10000 {
                        println!("\n10s passed since last packet ==> Contact broke. Trying to resend packet...");
                        if let Some(buf) = self.last_transmitted.get(&idn) {
                            loop {
                                match self.socket.send(buf) {
                                    Ok(x) => {
                                        if x != buf.len() {
                                            continue;
                                        }
                                    }
                                    Err(_) => {
                                        continue;
                                    }
                                }
                                break;
                            }
                            start = unix_millis();
                        } else {
                            break; // Latest packet was already ACK'd ==> No packets properly lost ==> Can continue with next packet.
                        }
                    }
                    if !wait {
                        break;
                    }
                }
            }
        }
        self.socket
            .set_read_timeout(Some(Duration::from_millis(1000)))
            .unwrap();
        Ok(())
    }
}
