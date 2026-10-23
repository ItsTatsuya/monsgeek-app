//! A single FIFO queue per connection. HID commands must not overtake profile switches.

use super::KeyboardClient;
use anyhow::{anyhow, Result};
use std::sync::mpsc;

type Job = Box<dyn FnOnce(&mut KeyboardClient) + Send>;

pub struct KeyboardWorker {
    sender: mpsc::Sender<Job>,
}

impl KeyboardWorker {
    pub fn new(mut client: KeyboardClient) -> Self {
        let (sender, receiver) = mpsc::channel::<Job>();
        std::thread::spawn(move || {
            while let Ok(job) = receiver.recv() {
                job(&mut client);
            }
        });
        Self { sender }
    }

    // Enqueue synchronously, before returning the future to GPUI. Executor scheduling
    // can then never change the order in which the keyboard sees these jobs.
    pub fn submit<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut KeyboardClient) -> Result<T> + Send + 'static,
    ) -> async_channel::Receiver<Result<T>> {
        let (sender, receiver) = async_channel::bounded(1);
        let failure = sender.clone();
        if self
            .sender
            .send(Box::new(move |client| {
                let _ = sender.try_send(job(client));
            }))
            .is_err()
        {
            let _ = failure.try_send(Err(anyhow!("Keyboard connection closed")));
        }
        receiver
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{devices, protocol::mock::MockTransport};
    use std::time::Duration;

    #[test]
    fn jobs_preserve_profile_order_even_when_a_result_is_discarded() {
        let def = devices::find(2304).unwrap();
        let worker = KeyboardWorker::new(KeyboardClient::new(Box::new(MockTransport::new(def))));
        let (finished, received) = mpsc::channel();
        drop(worker.submit(|kb| {
            std::thread::sleep(Duration::from_millis(30));
            kb.set_profile(1)
        }));
        drop(worker.submit(|kb| kb.set_key(1, 0, 9, [0, 0, 0x29, 0])));
        drop(worker.submit(move |kb| {
            let profile = kb.get_profile()?;
            let action = kb.get_key_matrix(profile, 0)?[9];
            finished.send((profile, action))?;
            Ok(())
        }));
        drop(worker); // Pending writes still finish before the client is released.
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            (1, [0, 0, 0x29, 0])
        );
    }
}
