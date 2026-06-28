#![no_std]
#![no_main]

use defmt::{info, error};
use defmt_rtt as _;
use defmt_rtt as _;
use embassy_executor::{Spawner, task};
use embassy_futures::yield_now;
use embassy_stm32::{bind_interrupts, exti::ExtiInput, mode::Async};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};
use embassy_time::{Timer, Duration};
use panic_probe as _;

use crate::{sht31::SHT31Reading, ssd1315::SSD1315};

mod animation;
mod main_menu;
mod ssd1315;
mod sht31;

const DISPLAY_WIDTH : u8 = 128;
const DISPLAY_HEIGHT : u8 = 64;

type SharedI2c = embassy_sync::mutex::Mutex<embassy_sync::blocking_mutex::raw::ThreadModeRawMutex,
    Option<embassy_stm32::i2c::I2c<'static, embassy_stm32::mode::Async, embassy_stm32::i2c::Master>>>;

#[derive(Clone, Copy)]
enum InputDirection {
    Up,
    Down,
}

static I2C : SharedI2c = embassy_sync::mutex::Mutex::new(Option::None);
static CLIMATE : Signal<ThreadModeRawMutex, SHT31Reading> = Signal::new();
static INPUT : Signal<ThreadModeRawMutex, InputDirection> = Signal::new();

bind_interrupts!(struct Irqs {
    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<embassy_stm32::peripherals::I2C1>;
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<embassy_stm32::peripherals::I2C1>;
    DMA1_CHANNEL6 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH6>;
    DMA1_CHANNEL7 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH7>;
    EXTI3 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI3>;
    EXTI4 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI4>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    use animation::*;

    let p = embassy_stm32::init(Default::default());

    let mut cfg = embassy_stm32::i2c::Config::default();
    cfg.frequency = embassy_stm32::time::Hertz(400_000);
    let i2c = embassy_stm32::i2c::I2c::new(p.I2C1, p.PB6, p.PB7,
        p.DMA1_CH6, p.DMA1_CH7, Irqs, cfg);
    let _ = I2C.lock().await.insert(i2c);

    let mut display = ssd1315::SSD1315::new();
    while let Err(_) = display.init().await {
        error!("Display not found, retrying...");
        Timer::after_secs(1).await;
    }

    let mut logo_anim : [Animations; _] = [LogoAnimation::new()];
    animate(&mut display, &mut logo_anim, Duration::from_millis(50)).await.unwrap();

    let down_btn = ExtiInput::new(p.PB3, p.EXTI3, embassy_stm32::gpio::Pull::Down, Irqs);
    let up_btn = ExtiInput::new(p.PB4, p.EXTI4, embassy_stm32::gpio::Pull::Down, Irqs);

    spawner.spawn(listen_input(down_btn, InputDirection::Down).unwrap());
    spawner.spawn(listen_input(up_btn, InputDirection::Up).unwrap());
    spawner.spawn(read_sht().unwrap());
    spawner.spawn(render_menu(display).unwrap());
    loop {
        yield_now().await;
    }
}

#[task(pool_size=2)]
async fn listen_input(
    mut btn : ExtiInput<'static, Async>,
    value : InputDirection,
) {
    loop {
        btn.wait_for_rising_edge().await;
        INPUT.signal(value);
        Timer::after_millis(200).await;
    }
}

#[task]
async fn render_menu(mut display : SSD1315) {
    use main_menu::*;
    let mut menu = MainMenu::new();
    loop {
        match INPUT.try_take() {
            Some(input) => menu.menu_set(input),
            None => ()
        }
        match CLIMATE.try_take() {
            Some(climate) => menu.set_climate(climate).await,
            None => (),
        }
        menu.tick(&mut display).await;
        Timer::after_millis(33).await;
    }
}

#[task]
async fn read_sht() {
    let sht31 = sht31::SHT31::new();

    loop {
        match sht31.get_climate().await {
            Ok(sht31_reading) => CLIMATE.signal(sht31_reading),
            Err(err) => error!("{:?}", err),
        }
        Timer::after_secs(5).await;
    }
}
