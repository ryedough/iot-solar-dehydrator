use embassy_time::{Duration, Instant};

pub mod main_menu;
pub mod sensor_menu;
pub mod fan_menu;
pub use main_menu::MainMenu;
pub use sensor_menu::SensorMenu;
pub use fan_menu::FanMenu;

use crate::{InputEvt, animation::FlushableDisplay};

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

