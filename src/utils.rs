use std::ops::Deref;

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
