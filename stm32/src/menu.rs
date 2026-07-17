use core::sync::atomic::Ordering;

use embassy_executor::task;
use embassy_stm32::{peripherals::TIM1, timer::simple_pwm::SimplePwm};
use embassy_time::{Duration, Instant, Timer};

pub mod main_menu;
pub mod sensor_menu;
pub mod fan_menu;
pub mod wifi_menu;
pub use main_menu::MainMenu;
pub use sensor_menu::SensorMenu;
pub use fan_menu::FanMenu;

use crate::{InputEvt, animation::FlushableDisplay, at24c08::{AT24C08, registered_addresses}, channeled_signal::{SignalReceiver, SignalSender}, menu::fan_menu::FanSpeed, sht31::SHT31Reading, ssd1315::SSD1315};

pub enum Menu {
    MainMenu(MainMenu),
    SensorMenu(SensorMenu),
    FanMenu(FanMenu),
}

impl Menu {
    pub async fn tick(&mut self, display: &mut impl FlushableDisplay) {
        match self {
            Menu::MainMenu(m) => m.tick(display).await,
            Menu::SensorMenu(m) => m.tick(display).await,
            Menu::FanMenu(m) => m.tick(display).await,
        }
    }
}

pub trait BareMenu {
    type OnInputReturn;
    async fn tick(&mut self, display: &mut impl FlushableDisplay);
    fn on_input(&mut self, evt: InputEvt) -> Self::OnInputReturn;
}

struct Lerp {
    a: f32,
    b: f32,
    start: Instant,
    duration: Duration,
}

impl Lerp {
    pub fn new(a: f32, b: f32, duration: Duration) -> Self {
        Self {
            a,
            b,
            start: Instant::now(),
            duration,
        }
    }
    pub fn get(&self) -> f32 {
        let mut t = self.start.elapsed().as_millis() as f32 / self.duration.as_millis() as f32;
        if t > 1. {
            t = 1.
        };
        self.b * t + self.a * (1. - t)
    }
    pub fn is_done(&self) -> bool {
        self.start.elapsed() > self.duration
    }
}


#[task]
pub async fn render_menu_task(
    mut display: SSD1315,
    eeprom: AT24C08,
    mut pwm: SimplePwm<'static, TIM1>,
    input_sr: SignalReceiver<InputEvt>,
    climate_sr: SignalReceiver<SHT31Reading>,
    calibration_ss: SignalSender<SHT31Reading>,
    mut calibration: SHT31Reading,
    mut fan_speed: FanSpeed,
) {
    let mut menu = Menu::MainMenu(MainMenu::new(None, None));
    let mut saved_climate = None;
    loop {
        let input_flag = input_sr.try_receive();
        match &mut menu {
            Menu::MainMenu(m) => {
                use main_menu::OnInputFlag;
                match climate_sr.try_receive() {
                    Some(climate) => {
                        m.set_climate(climate);
                        saved_climate = Some(climate);
                    }
                    None => {}
                }
                let input_flag = input_flag.map(|e| m.on_input(e));
                match input_flag {
                    Some(f) => match f {
                        OnInputFlag::ToSensorMenu => {
                            menu = Menu::SensorMenu(SensorMenu::new(calibration));
                        }
                        OnInputFlag::ToFanMenu => menu = Menu::FanMenu(FanMenu::new(fan_speed)),
                        OnInputFlag::None => (),
                    },
                    None => (),
                };
            }
            Menu::FanMenu(m) => {
                use fan_menu::OnInputFlag;
                let input_flag = input_flag.map(|e| m.on_input(e));
                match input_flag {
                    Some(f) => {
                        match f {
                            OnInputFlag::BackToMenu => {
                                menu = Menu::MainMenu(MainMenu::new(
                                    Some(main_menu::Selection::Fan),
                                    saved_climate,
                                ))
                            }
                            OnInputFlag::Save(new_fan_speed) => {
                                eeprom
                                    .write(&registered_addresses::FAN_SPEED, &new_fan_speed)
                                    .await
                                    .unwrap();
                                fan_speed = new_fan_speed;
                                pwm.ch1().set_duty_cycle_percent(new_fan_speed.as_percent());
                                // FAN_SPEED.signal(new_fan_speed);
                                menu = Menu::MainMenu(MainMenu::new(
                                    Some(main_menu::Selection::Fan),
                                    saved_climate,
                                ));
                            }
                            OnInputFlag::None => (),
                        }
                    }
                    None => {}
                }
            }
            Menu::SensorMenu(m) => {
                use sensor_menu::OnInputFlag;
                let input_flag = input_sr.try_receive().map(|e| m.on_input(e));
                match input_flag {
                    Some(f) => match f {
                        OnInputFlag::Save(new_calibration) => {
                            eeprom
                                .write(&registered_addresses::SHT_CALIBRATION, &new_calibration)
                                .await
                                .unwrap();
                            calibration = new_calibration.clone();
                            calibration_ss.send(new_calibration);
                            menu = Menu::MainMenu(MainMenu::new(
                                Some(main_menu::Selection::Sensor),
                                saved_climate,
                            ));
                        }
                        OnInputFlag::BackToMain => {
                            menu = Menu::MainMenu(MainMenu::new(
                                Some(main_menu::Selection::Sensor),
                                saved_climate,
                            ))
                        }
                        OnInputFlag::None => (),
                    },
                    None => {}
                }
            }
        }
        menu.tick(&mut display).await;
        Timer::after_millis(33).await;
    }
}
