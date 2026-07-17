use embassy_executor::{Spawner, task};
use embassy_futures::yield_now;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, channel::{Channel, Receiver, Sender}};
use embassy_time::Instant;

use crate::{esp12f::*, sht31::SHT31Reading};

const ESP12F_CONSUMER_LEN : usize = 2;

pub enum ESP12FResult {
    Scan(ScanReturn),
    Connect(ConnectReturn),
    SendClimate(SendClimateReturn),
}

pub enum ESP12Command {
    Scan,
    Connect(ConnWifi),
    SendClimate(SHT31Reading),
}

pub struct ESP12FConsumer {
    recv : Receiver<'static, ThreadModeRawMutex, ESP12FResult, 1>,
    send : Sender<'static, ThreadModeRawMutex, ESP12Command, 1>,
}

impl ESP12FConsumer {
    pub async fn scan(&self)-> ScanReturn {
        self.send.send(ESP12Command::Scan).await;
        let recv = self.recv.receive().await;
        if let ESP12FResult::Scan(res) = recv {
            return res
        } else {
            panic!("this shouldnt possible unless you messed up esp12 task");
        }
    }
    pub async fn connect(&self, wifi : ConnWifi)->ConnectReturn {
        self.send.send(ESP12Command::Connect(wifi)).await;
        let recv = self.recv.receive().await;
        if let ESP12FResult::Connect(res) = recv {
            return res
        } else {
            panic!("this shouldnt possible unless you messed up esp12 task");
        }
    }
}

struct ESP12FProducer{
    send : Sender<'static, ThreadModeRawMutex, ESP12FResult, 1>,
    recv : Receiver<'static, ThreadModeRawMutex, ESP12Command, 1>,
}

pub struct ESP12FTaskMaker{
    result_ch : [Channel::<ThreadModeRawMutex, ESP12FResult, 1>; ESP12F_CONSUMER_LEN],
    command_ch: [Channel::<ThreadModeRawMutex, ESP12Command, 1>; ESP12F_CONSUMER_LEN]
}
impl ESP12FTaskMaker {
    pub const fn new(result_ch : [Channel::<ThreadModeRawMutex, ESP12FResult, 1>; ESP12F_CONSUMER_LEN], command_ch : [Channel::<ThreadModeRawMutex, ESP12Command, 1>; ESP12F_CONSUMER_LEN])->Self {
        Self {
            result_ch,
            command_ch,
        }
    }
    /// Make sure to call this function ONLY once through program runtime
    pub fn create_esp12f_execute_command_task(&'static self, spawner : &Spawner, esp12f : ESP12F)-> [ESP12FConsumer; ESP12F_CONSUMER_LEN]{
        let consoomer : [ESP12FConsumer; ESP12F_CONSUMER_LEN] = core::array::from_fn(|i|ESP12FConsumer {
                recv : self.result_ch[i].receiver(),
                send : self.command_ch[i].sender(),
            });
        let producers : [ESP12FProducer; ESP12F_CONSUMER_LEN] = core::array::from_fn(|i|ESP12FProducer {
                recv : self.command_ch[i].receiver(),
                send : self.result_ch[i].sender(),
            });
        spawner.spawn(esp12f_execute_command_task(esp12f, producers).unwrap());
        consoomer
    }
}

const MAX_LAST_COMMAND_DURATION :Duration = Duration::from_millis(100);
#[task]
async fn esp12f_execute_command_task(mut esp12f : ESP12F, sender : [ESP12FProducer; ESP12F_CONSUMER_LEN]) -> !{
    let mut last_command = Instant::now();
    loop {
        for s in &sender {
            match s.recv.try_receive() {
                Ok(command) => {
                    last_command = Instant::now();
                    let esp12f = esp12f.borrow_active_mut().await;
                    let result = match command {
                        ESP12Command::Scan => ESP12FResult::Scan(
                               esp12f.scan().await
                            ),
                        ESP12Command::Connect(wifi) => ESP12FResult::Connect(
                                esp12f.connect(wifi).await
                            ),
                        ESP12Command::SendClimate(reading) => ESP12FResult::SendClimate(
                                Ok(())
                            )
                    };
                    s.send.send(result).await;
                },
                Err(_) => {
                    yield_now().await;
                    if last_command.elapsed() > MAX_LAST_COMMAND_DURATION {
                        esp12f.turn_off().await;
                    }
                },
            }
        }

    }
}

