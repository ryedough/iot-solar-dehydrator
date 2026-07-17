use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};

pub struct ChanneledSignal<T: 'static>{
    signal : Signal<ThreadModeRawMutex, T>,
}

pub struct SignalReceiver<T : 'static> {
    signal : &'static Signal<ThreadModeRawMutex, T>,
}
impl<T> SignalReceiver<T> {
    pub async fn receive(&self)->T {
        self.signal.wait().await
    }
    pub fn try_receive(&self)->Option<T> {
        self.signal.try_take()
    }
}

pub struct SignalSender<T : 'static>{
    signal : &'static Signal<ThreadModeRawMutex, T>,
}
impl<T> SignalSender<T> {
    pub fn send(&self, val : T){
        self.signal.signal(val);
    }
}

impl<T> ChanneledSignal<T> {
    pub const fn new()->Self{
        Self{
            signal: Signal::new()
        }
    }
    /// make sure to call this only once during lifetime
    pub fn receiver(&'static self)->SignalReceiver<T> {
        SignalReceiver::<T>{
            signal : &self.signal
        }
    }
    pub fn sender(&'static self)->SignalSender<T>{
        SignalSender::<T>{
            signal : &self.signal
        }
    }
}
