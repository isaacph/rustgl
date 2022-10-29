use std::{time::{Instant, Duration}, fs::{File, OpenOptions}, io::{self, Write}};

pub struct TimerLogging {
    out: Vec<String>,
    file: File,
}

impl TimerLogging {
    pub fn init(file_name: &str) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).write(true).open(file_name)?;
        Ok(Self {
            file,
            out: vec![]
        })
    }
    pub fn log(&mut self, title: &str, time: Duration) {
        self.out.push(format!("{} {}", title, time.as_micros()));
    }
    pub fn flush(&mut self) {
        writeln!(self.file, "{}",
            self.out.drain(0..self.out.len())
            .fold(String::new(), |acc, s| format!("{}\n{}", acc, s)))
            .map_err(|err| println!("Error logging: {}", err)).ok();
    }
}

pub struct TimerData {
    pub title: String,
    pub start: Instant,
}

pub struct Timer {
    pub data: Option<TimerData>
}

#[cfg(feature = "timing")]
impl Timer {
    pub fn start(title: &str) -> Self {
        Self {
            data: Some(TimerData {
                title: String::from(title),
                start: Instant::now(),
            })
        }
    }

    pub fn log(&self, logging: &mut TimerLogging) {
        self.data.as_ref().map(|data| logging.log(data.title.as_str(), data.start.elapsed()));
    }
}

#[cfg(not(feature = "timing"))]
impl Timer {
    pub fn start(_: &str) -> Self {
        Self { data: None }
    }
    pub fn log(&self, _: &mut TimerLogging) {
    }
}
