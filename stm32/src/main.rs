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

use crate::{at24c08::registered_addresses, sht31::SHT31Reading, ssd1315::SSD1315};

mod animation;
mod menu;
mod ssd1315;
mod sht31;
mod at24c08;

bind_interrupts!(struct Irqs {
    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<embassy_stm32::peripherals::I2C1>;
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<embassy_stm32::peripherals::I2C1>;
    DMA1_CHANNEL6 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH6>;
    DMA1_CHANNEL7 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH7>;
    EXTI0 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI0>;
    EXTI1 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI1>;
    EXTI3 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI3>;
    EXTI4 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI4>;
});

#[derive(Clone, Copy)]
enum InputEvt {
    Up,
    Down,
    Enter,
}

const DISPLAY_WIDTH : u8 = 128;
const DISPLAY_HEIGHT : u8 = 64;

type SharedI2c = embassy_sync::mutex::Mutex<embassy_sync::blocking_mutex::raw::ThreadModeRawMutex,
    Option<embassy_stm32::i2c::I2c<'static, embassy_stm32::mode::Async, embassy_stm32::i2c::Master>>>;
static I2C : SharedI2c = embassy_sync::mutex::Mutex::new(Option::None);
static CLIMATE : Signal<ThreadModeRawMutex, SHT31Reading> = Signal::new();
static INPUT : Signal<ThreadModeRawMutex, InputEvt> = Signal::new();
static CALIBRATION : Signal<ThreadModeRawMutex, SHT31Reading> = Signal::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    use animation::*;
    let p = embassy_stm32::init(Default::default());

    // init i2c
    let mut cfg = embassy_stm32::i2c::Config::default();
    cfg.frequency = embassy_stm32::time::Hertz(400_000);
    let i2c = embassy_stm32::i2c::I2c::new(p.I2C1, p.PB6, p.PB7,
        p.DMA1_CH6, p.DMA1_CH7, Irqs, cfg);
    let _ = I2C.lock().await.insert(i2c);

    // init eeprom and load settings
    let mut eeprom = at24c08::AT24C08::new();
    let calibration = eeprom.read(registered_addresses::SHT_CALIBRATION);

    // init display
    let mut display = ssd1315::SSD1315::new();
    while let Err(_) = display.init().await {
        error!("Display not found, retrying...");
        Timer::after_secs(1).await;
    }
    // animate logo splash screen
    let mut logo_anim : [Animations; _] = [LogoAnimation::new()];
    let logo_anim = animate(&mut display, &mut logo_anim, Duration::from_millis(50));

    // join read setting and animate logo
    let (calibration, logo_anim)= embassy_futures::join::join(calibration, logo_anim).await;
    let calibration = calibration.unwrap();
    // on first boot, sht reading will be nan
    let calibration = if calibration.temp.is_nan() || calibration.humid.is_nan() {
        eeprom.write(registered_addresses::SHT_CALIBRATION, SHT31Reading{
            temp : 0.,
            humid : 0.,
        }).await.unwrap();
        SHT31Reading {
            humid : 0.,
            temp : 0.,
        }
    } else {calibration};
    logo_anim.unwrap();
    CALIBRATION.signal(calibration.clone());

    // init button
    let down_btn = ExtiInput::new(p.PB3, p.EXTI3, embassy_stm32::gpio::Pull::Down, Irqs);
    let up_btn = ExtiInput::new(p.PB4, p.EXTI4, embassy_stm32::gpio::Pull::Down, Irqs);
    let enter_btn = ExtiInput::new(p.PB0, p.EXTI0, embassy_stm32::gpio::Pull::Down, Irqs);
    spawner.spawn(listen_input(down_btn, InputEvt::Down).unwrap());
    spawner.spawn(listen_input(up_btn, InputEvt::Up).unwrap());
    spawner.spawn(listen_input(enter_btn, InputEvt::Enter).unwrap());

    // init task
    spawner.spawn(read_sht().unwrap());
    spawner.spawn(render_menu(display, calibration).unwrap());
    loop {
        yield_now().await;
    }
}

#[task(pool_size=4)]
async fn listen_input(
    mut btn : ExtiInput<'static, Async>,
    value : InputEvt,
) {
    loop {
        btn.wait_for_rising_edge().await;
        INPUT.signal(value);
        Timer::after_millis(200).await;
    }
}

#[task]
async fn render_menu(mut display : SSD1315, mut calibration : SHT31Reading) {
    use menu::*;
    let mut menu = Menu::MainMenu(MainMenu::new());
    loop {
        match &mut menu {
            Menu::MainMenu(m) => {
                match CLIMATE.try_take() {
                    Some(climate) => m.set_climate(climate).await,
                    None => (),
                }
                let input_flag = INPUT.try_take().map(|e| m.on_input(e));
                match input_flag {
                    Some(f) => match f {
                        menu::main_menu::OnInputFlag::ToSensorMenu => {
                            menu = Menu::SensorMenu(SensorMenu::new(calibration.clone()));
                        },
                        menu::main_menu::OnInputFlag::None => (),
                    },
                    None => (),
                }

            },
            Menu::SensorMenu(m) => {
                let input_flag = INPUT.try_take().map(|e| m.on_input(e));
                match input_flag {
                    Some(f) => {
                        match f {
                            menu::sensor_menu::OnInputFlag::Save(r) => calibration = r,
                            menu::sensor_menu::OnInputFlag::None => ()
                        }
                    },
                    None => {},
                }
            }
        }
        menu.tick(&mut display).await;
        Timer::after_millis(33).await;
    }
}

#[task]
async fn read_sht() {
    let calibration = CALIBRATION.wait().await;
    let mut sht31 = sht31::SHT31::new(calibration);

    loop {
        if let Some(calibration) = CALIBRATION.try_take() {
            sht31 = sht31::SHT31::new(calibration);
        }
        match sht31.get_climate().await {
            Ok(sht31_reading) => CLIMATE.signal(sht31_reading),
            Err(err) => error!("{:?}", err),
        }
        Timer::after_secs(5).await;
    }
}
