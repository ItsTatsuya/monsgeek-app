//! Real keyboards over hidapi: the config collection (feature reports) and the notify collection (travel stream).

use super::{
    parse_travel_report, parse_vendor_report, Transport, VendorEvent, CONFIG_USAGE, CONFIG_USAGE_PAGE, NOTIFY_USAGE, REPORT_SIZE,
    VENDOR_ID,
};
use anyhow::{anyhow, Context, Result};
use hidapi::{HidApi, HidDevice};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub struct HidTransport {
    dev: HidDevice,
    #[allow(dead_code)]
    product: String,
}

impl HidTransport {
    /// Open the first MonsGeek/Akko config collection (VID 0x3151, usage page 0xFFFF, usage 2).
    pub fn open(api: &HidApi) -> Result<Self> {
        let info = api
            .device_list()
            .find(|d| {
                d.vendor_id() == VENDOR_ID
                    && d.usage_page() == CONFIG_USAGE_PAGE
                    && d.usage() == CONFIG_USAGE
            })
            .ok_or_else(|| {
                anyhow!("No MonsGeek keyboard found. Plug it in with the USB cable and try again.")
            })?;
        let product = info
            .product_string()
            .unwrap_or("MonsGeek keyboard")
            .to_string();
        let dev = info
            .open_device(api)
            .context("Couldn't open the keyboard. Close the official driver and try again.")?;
        Ok(Self { dev, product })
    }
}

impl Transport for HidTransport {
    fn send_feature(&mut self, data: &[u8; REPORT_SIZE]) -> Result<()> {
        let mut buf = [0u8; REPORT_SIZE + 1];
        buf[1..].copy_from_slice(data);
        self.dev
            .send_feature_report(&buf)
            .context("Sending to the keyboard failed")?;
        Ok(())
    }

    fn receive_feature(&mut self) -> Result<[u8; REPORT_SIZE]> {
        let mut buf = [0u8; REPORT_SIZE + 1];
        let n = self
            .dev
            .get_feature_report(&mut buf)
            .context("Reading from the keyboard failed")?;
        let mut out = [0u8; REPORT_SIZE];
        // Windows returns the report id first; skip it when present.
        let src = match n {
            n if n == REPORT_SIZE + 1 => &buf[1..],
            REPORT_SIZE => &buf[..REPORT_SIZE],
            _ => return Err(anyhow!("Incomplete keyboard reply: {n} bytes")),
        };
        out.copy_from_slice(src);
        Ok(out)
    }

    fn name(&self) -> String {
        self.product.clone()
    }
}

/// Latest travel per matrix key in raw steps, filled by a stream reader.
pub type TravelBuffer = Arc<Mutex<Vec<u16>>>;

/// Reads the notify collection on a background thread for the whole connection:
/// `[5, 27, lo, hi, index]` travel reports fill the buffers (the caller turns the stream on and
/// off with `KeyboardClient::set_travel_stream`); other notifications go to `events`.
pub struct TravelStream {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub buffer: TravelBuffer,
    pub peaks: TravelBuffer,
    /// Reports received since start (for rate display).
    pub count: Arc<Mutex<u64>>,
}

impl TravelStream {
    pub fn open(api: &HidApi, events: async_channel::Sender<VendorEvent>) -> Result<Self> {
        let info = api
            .device_list()
            .find(|d| {
                d.vendor_id() == VENDOR_ID
                    && d.usage_page() == CONFIG_USAGE_PAGE
                    && d.usage() == NOTIFY_USAGE
            })
            .ok_or_else(|| anyhow!("The keyboard's notify collection wasn't found"))?;
        let dev = info
            .open_device(api)
            .context("Couldn't open the keyboard's notify collection")?;
        Ok(Self::spawn(
            move |buf| dev.read_timeout(buf, 50).map_err(|e| anyhow!(e)),
            Some(events),
        ))
    }

    /// Run a reader over any source of input reports (the mock uses this too).
    pub fn spawn(
        mut read: impl FnMut(&mut [u8]) -> Result<usize> + Send + 'static,
        events: Option<async_channel::Sender<VendorEvent>>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let buffer: TravelBuffer = Arc::new(Mutex::new(vec![0; super::MATRIX_KEYS]));
        let peaks: TravelBuffer = Arc::new(Mutex::new(vec![0; super::MATRIX_KEYS]));
        let count = Arc::new(Mutex::new(0u64));
        let (s, b, c) = (stop.clone(), buffer.clone(), count.clone());
        let p = peaks.clone();
        let thread = std::thread::spawn(move || {
            let mut buf = [0u8; REPORT_SIZE + 1];
            while !s.load(Ordering::Relaxed) {
                match read(&mut buf) {
                    Ok(0) => {}
                    Ok(n) => {
                        if let Some((i, v)) = parse_travel_report(&buf[..n]) {
                            b.lock().unwrap()[i] = v;
                            let mut peaks = p.lock().unwrap();
                            peaks[i] = peaks[i].max(v);
                            *c.lock().unwrap() += 1;
                        } else if let (Some(e), Some(tx)) = (parse_vendor_report(&buf[..n]), &events) {
                            let _ = tx.try_send(e);
                        }
                    }
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
                }
            }
        });
        Self {
            stop,
            thread: Some(thread),
            buffer,
            peaks,
            count,
        }
    }
}

impl Drop for TravelStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn peaks_keep_reports_between_ui_frames() {
        let (done, received) = mpsc::channel();
        let mut report = 0;
        let stream = TravelStream::spawn(
            move |buf| {
            report += 1;
            match report {
                1 => {
                    buf[..5].copy_from_slice(&[5, 27, 94, 1, 14]);
                    Ok(5)
                }
                2 => {
                    buf[..5].copy_from_slice(&[5, 27, 0, 0, 14]);
                    Ok(5)
                }
                _ => {
                    if report == 3 {
                        done.send(()).unwrap();
                    }
                    std::thread::sleep(Duration::from_millis(2));
                    Ok(0)
                }
            }
        },
            None,
        );
        received.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(stream.buffer.lock().unwrap()[14], 0);
        assert_eq!(stream.peaks.lock().unwrap()[14], 350);
        assert_eq!(*stream.count.lock().unwrap(), 2);
    }
}
