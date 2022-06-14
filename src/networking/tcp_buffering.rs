use std::{collections::VecDeque, cmp};

use super::config::{MAX_TCP_MESSAGE_SIZE, MAX_TCP_MESSAGE_QUEUE_SIZE};


pub struct TcpRecvState {
    buffer: Box<[u8]>,
    length: usize,
    remaining: usize,
    failure: Option<String>
}

impl TcpRecvState {
    pub fn init() -> TcpRecvState {
        TcpRecvState {
            buffer: vec![0u8; MAX_TCP_MESSAGE_SIZE].into_boxed_slice(),
            length: 0,
            remaining: 0,
            failure: None
        }
    }

    pub fn failed(&self) -> Option<String> {
        self.failure.clone()
    }

    pub fn receive(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut messages: Vec<Vec<u8>> = vec![];

        let mut extension_read_start = 0;
        let extension: &[u8] = data;

        // work until reaching the end of the current extension of data
        while extension_read_start < extension.len() {
            // we need to find another remaining number of bytes to process
            if self.remaining == 0 {
                if self.length + extension.len() - extension_read_start < 4 {
                    // copy the rest of extension in
                    let copy_len = extension.len() - extension_read_start;
                    self.buffer[self.length..self.length + copy_len]
                        .copy_from_slice(&extension[extension_read_start..]);
                        self.length += copy_len;
                    //extension_read_start += copy_len; // will not be read again
                    break;

                } else if self.length >= 4 {
                    // this shouldn't happen
                    panic!("Logic error decoding TCP message!");

                } else {
                    // we have enough bytes to find message length
                    // remove remaining message length bytes from extension
                    let copy_len = 4 - self.length;
                    self.buffer[self.length..self.length + copy_len].copy_from_slice(
                        &extension[extension_read_start..extension_read_start + copy_len]);
                    //self.length += extend_by; // will be cleared anyway
                    extension_read_start += copy_len;

                    // find full message length
                    let remaining_in_bytes: [u8; 4] = self.buffer[0..4].try_into().unwrap();
                    self.remaining = u32::from_be_bytes(remaining_in_bytes) as usize;

                    // clear message buffer (should have only 4 bytes)
                    self.length = 0;
                }
            }
            if self.remaining > MAX_TCP_MESSAGE_SIZE {
                self.failure = Some(format!("Attempted to decode message that was too big: {}", self.remaining));
                break;
            }
            if self.remaining > 0 {
                // copy either remaining message bytes, or all new bytes, whichever is smaller
                let copy_len = cmp::min(self.remaining, extension.len() - extension_read_start);
                self.buffer[self.length..self.length + copy_len]
                    .copy_from_slice(&extension[extension_read_start..extension_read_start + copy_len]);
                self.length += copy_len;
                self.remaining -= copy_len;
                extension_read_start += copy_len;

                // if we copied all remaining message bytes
                if self.remaining == 0 {
                    // we finished a packet!
                    // store the finished message
                    let finished_message = self.buffer[0..self.length].to_vec();
                    messages.push(finished_message);
                    // clear the message
                    self.length = 0;
                }
            }
        }
        messages
    }
}

pub struct TcpSendState {
    queue: VecDeque<Vec<u8>>,
    queue_size: usize,
    buffer: Box<[u8]>,
    length: usize,
    position: usize
}

impl TcpSendState {
    pub fn init() -> Self {
        TcpSendState {
            queue: VecDeque::new(),
            queue_size: 0,
            buffer: vec![0u8; MAX_TCP_MESSAGE_SIZE].into_boxed_slice(),
            length: 0,
            position: 0
        }
    }

    pub fn next_send(&self) -> Option<&[u8]> {
        if self.position < self.length {
            Some(&self.buffer[self.position..self.length])
        } else {
            None
        }
    }

    pub fn update_buffer(&mut self, sent: usize) {
        self.position += sent;
        if self.position >= self.length {
            if let Some(next) = self.queue.pop_front() {
                let length: [u8; 4] = u32::to_be_bytes(next.len() as u32);
                self.buffer[0..4].copy_from_slice(&length);
                self.buffer[4..4 + next.len()].copy_from_slice(next.as_slice());
                self.position = 0;
                self.length = next.len() + 4;
                self.queue_size -= next.len();
            }
        }
    }

    pub fn enqueue(&mut self, packet: Vec<u8>) -> std::result::Result<(), String> {
        if packet.len() + 4 > MAX_TCP_MESSAGE_SIZE {
            Err(format!("Tried to send message that was too big: {} > {}", packet.len() + 4, MAX_TCP_MESSAGE_SIZE))
        } else if self.queue_size + packet.len() > MAX_TCP_MESSAGE_QUEUE_SIZE {
            Err(format!("Exceeded maximum message queue size for client"))
        } else {
            self.queue_size += packet.len();
            self.queue.push_back(packet);
            self.update_buffer(0);
            Ok(())
        }
    }
}
