pub const MAX_UDP_MESSAGE_SIZE: usize = 512;
pub const MAX_TCP_MESSAGE_SIZE: usize = 1<<20; // why would you send more than 1MB? even that's probably too much
pub const MAX_TCP_MESSAGE_QUEUE_SIZE: usize = 1<<26; // max they can ddos me for 640 mb