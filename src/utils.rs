use async_std::task;
use cfg_if::cfg_if;
use std::future::Future;
use std::ops::Deref;

pub fn unbounded_channel<T>() -> (Sender<T>, Receiver<T>) {
    let (s, r) = async_channel::unbounded();
    (Sender::new(s), Receiver::new(r))
}

pub struct Sender<T>(async_channel::Sender<T>);

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for Sender<T> {
    type Target = async_channel::Sender<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Sender<T> {
    pub fn new(sender: async_channel::Sender<T>) -> Self {
        Self(sender)
    }
}

pub struct Receiver<T>(async_channel::Receiver<T>);

impl<T> Deref for Receiver<T> {
    type Target = async_channel::Receiver<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Receiver<T> {
    pub fn new(receiver: async_channel::Receiver<T>) -> Self {
        Self(receiver)
    }
}

pub fn block_on(f: impl Future<Output = ()> + 'static) {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            // On wasm, astnc_std::task::block_on does not block.
            task::block_on(f);
        } else {
            task::block_on(f)
        }
    }
}
