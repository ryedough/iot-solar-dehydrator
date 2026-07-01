#![no_std]
#![no_main]

use defmt::{info, error};
use defmt_rtt as _;
use defmt_rtt as _;
use embassy_executor::{Spawner, task};
use embassy_futures::{join, select::{Either, Select, select}, yield_now};
use embassy_stm32::{bind_interrupts, exti::ExtiInput, i2c, mode::Async};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};
use embassy_time::{Duration, Timer, WithTimeout};
use embedded_graphics::{pixelcolor::BinaryColor, primitives::PrimitiveStyle};
use panic_probe as _;

use crate::{at24c08::{AT24C08, AT24C08Error, registered_addresses}, menu::fan_menu::FanSpeed, sht31::SHT31Reading, ssd1315::SSD1315};

mod animation;
mod menu;
mod ssd1315;
mod sht31;
mod at24c08;
mod rotary_encoder;

bind_interrupts!(struct Irqs {
    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<embassy_stm32::peripherals::I2C1>;
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<embassy_stm32::peripherals::I2C1>;
    DMA1_CHANNEL6 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH6>;
    DMA1_CHANNEL7 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH7>;
    EXTI0 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI0>;
    EXTI3 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI3>;
    EXTI4 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI4>;
});

#[derive(Clone, Copy)]
enum InputEvt {
    CounterClockwise,
    Clockwise,
    Enter,
}

const DISPLAY_WIDTH : u8 = 128;
const DISPLAY_HEIGHT : u8 = 64;


const PRIMITIVE_STYLE_ON : PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_fill(BinaryColor::On);
const PRIMITIVE_STYLE_OFF : PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_fill(BinaryColor::Off);
const PRIMITIVE_STYLE_BORDER_ONLY : PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

type SharedI2c = embassy_sync::mutex::Mutex<embassy_sync::blocking_mutex::raw::ThreadModeRawMutex,
    Option<embassy_stm32::i2c::I2c<'static, embassy_stm32::mode::Async, embassy_stm32::i2c::Master>>>;
static I2C : SharedI2c = embassy_sync::mutex::Mutex::new(Option::None);
static CLIMATE : Signal<ThreadModeRawMutex, SHT31Reading> = Signal::new();
static INPUT : Signal<ThreadModeRawMutex, InputEvt> = Signal::new();
static FAN_SPEED : Signal<ThreadModeRawMutex, FanSpeed> = Signal::new();
static CALIBRATION : Signal<ThreadModeRawMutex, SHT31Reading> = Signal::new();

struct Settings {
    calibration : SHT31Reading,
    fan_speed : FanSpeed,
    eeprom : AT24C08,
}
impl Settings {
    pub fn new(eeprom : AT24C08, calibration : SHT31Reading, fan_speed : FanSpeed)->Self{
        Self { calibration, fan_speed, eeprom}
    }
}
async fn load_setting() -> Settings{
    let mut eeprom = at24c08::AT24C08::new();
    let mut calibration= eeprom.read(registered_addresses::SHT_CALIBRATION).await;
    let mut fan_speed = eeprom.read(registered_addresses::FAN_SPEED).await;
    while let Err(e) = calibration {
        match e {
            AT24C08Error::ConversionError => {
                let default = SHT31Reading::default();
                eeprom.write(registered_addresses::SHT_CALIBRATION, default.clone());
                calibration = Ok(default);
            },
            AT24C08Error::I2CError(_) => {
                error!("eeprom is not connected, trying again after 5 secs");
                Timer::after_secs(5).await;
                continue;
            }
        }
    }
    while let Err(e) = fan_speed {
        match e {
            AT24C08Error::ConversionError => {
                let default = FanSpeed::Medium;
                eeprom.write(registered_addresses::FAN_SPEED, default);
                fan_speed = Ok(default)
            },
            AT24C08Error::I2CError(_) => {
                error!("eeprom is not connected, trying again after 5 secs");
                Timer::after_secs(5).await;
                continue;
            }
        }
    }
    Settings::new(eeprom,calibration.unwrap(), fan_speed.unwrap())
}

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

    // init display
    let mut display = ssd1315::SSD1315::new();
    while let Err(_) = display.init().await {
        error!("Display not found, retrying...");
        Timer::after_secs(5).await;
    }

    // animate logo splash screen
    let mut logo_anim = LogoAnimation::new();

    let (settings, logo_anim)= embassy_futures::join::join(load_setting(), logo_anim.animate(&mut display, Duration::from_millis(50))).await;
    // on first boot, sht reading will be nan
    logo_anim.unwrap();
    CALIBRATION.signal(settings.calibration.clone());

    // init button
    let pin_a = ExtiInput::new(p.PB3, p.EXTI3, embassy_stm32::gpio::Pull::None, Irqs);
    let pin_b = ExtiInput::new(p.PB4, p.EXTI4, embassy_stm32::gpio::Pull::None, Irqs);
    let enter_btn = ExtiInput::new(p.PB0, p.EXTI0, embassy_stm32::gpio::Pull::Down, Irqs);
    // spawner.spawn(listen_input(down_btn, InputEvt::Down).unwrap());
    // spawner.spawn(listen_input(up_btn, InputEvt::Up).unwrap());
    // spawner.spawn(listen_input(enter_btn, InputEvt::Enter).unwrap());
    spawner.spawn(rotary_encoder(pin_a, pin_b).unwrap());
    spawner.spawn(listen_input(enter_btn, InputEvt::Enter).unwrap());

    // init task
    spawner.spawn(read_sht().unwrap());
    spawner.spawn(render_menu(display, settings.eeprom, settings.calibration, settings.fan_speed).unwrap());
    loop {
        yield_now().await;
    }
}

#[task]
async fn rotary_encoder(
    a : ExtiInput<'static, Async>,
    b : ExtiInput<'static, Async>,
    ){
    let mut encoder = rotary_encoder::RotaryEncoder::new(a, b);
    let mut counter = 0;
    let mut counted_dir = rotary_encoder::Direction::Clockwise;
    loop {
        match encoder.wait_direction().await {
            rotary_encoder::Direction::Clockwise => if counted_dir == rotary_encoder::Direction::Clockwise {
                counter+=1;
                if counter > 2 {
                    INPUT.signal(InputEvt::Clockwise);
                    counter = 0;
                }
            } else {
                counted_dir = rotary_encoder::Direction::Clockwise;
                counter=0;
            }
            rotary_encoder::Direction::CounterClockwise => if counted_dir == rotary_encoder::Direction::CounterClockwise {
                counter+=1;
                if counter > 2 {
                    INPUT.signal(InputEvt::CounterClockwise);
                    counter = 0;
                }
            } else {
                counted_dir = rotary_encoder::Direction::CounterClockwise;
                counter=0;
            }

        }
    }
}

#[task]
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
async fn render_menu(mut display : SSD1315, mut eeprom : AT24C08, mut calibration : SHT31Reading, mut fan_speed : FanSpeed) {
    use menu::*;
    let mut menu = Menu::MainMenu(MainMenu::new(None));
    loop {
        match &mut menu {
            Menu::MainMenu(m) => {
                use menu::main_menu::OnInputFlag;
                match CLIMATE.try_take() {
                    Some(climate) => m.set_climate(climate).await,
                    None => (),
                }
                let input_flag = INPUT.try_take().map(|e| m.on_input(e));
                match input_flag {
                    Some(f) => match f {
                        OnInputFlag::ToSensorMenu => {
                            menu = Menu::SensorMenu(SensorMenu::new(calibration.clone()));
                        },
                        OnInputFlag::ToFanMenu => {menu = Menu::FanMenu(FanMenu::new(fan_speed))},
                        OnInputFlag::None => (),
                    },
                    None => (),
                };
            },
            Menu::FanMenu(m) => {
                use menu::fan_menu::OnInputFlag;
                let input_flag = INPUT.try_take().map(|e| m.on_input(e));
                match input_flag {
                    Some(f) => {
                        match f {
                            OnInputFlag::BackToMenu => menu = Menu::MainMenu(MainMenu::new(Some(menu::main_menu::Selection::Fan))),
                            OnInputFlag::Save(new_fan_speed) => {
                                eeprom.write(registered_addresses::FAN_SPEED, new_fan_speed);
                                fan_speed = new_fan_speed;
                                FAN_SPEED.signal(new_fan_speed);
                                menu = Menu::MainMenu(MainMenu::new(Some(menu::main_menu::Selection::Fan)));
                            },
                            OnInputFlag::None => ()
                        }
                    },
                    None => {},
                }
            }
            Menu::SensorMenu(m) => {
                use menu::sensor_menu::OnInputFlag;
                let input_flag = INPUT.try_take().map(|e| m.on_input(e));
                match input_flag {
                    Some(f) => {
                        match f {
                            OnInputFlag::Save(new_calibration) => {
                                eeprom.write(registered_addresses::SHT_CALIBRATION,new_calibration.clone()).await.unwrap();
                                calibration = new_calibration.clone();
                                CALIBRATION.signal(new_calibration.clone());
                                menu = Menu::MainMenu(MainMenu::new(Some(menu::main_menu::Selection::Sensor)));
                            },
                            OnInputFlag::BackToMain => menu = Menu::MainMenu(MainMenu::new(Some(menu::main_menu::Selection::Sensor))),
                            OnInputFlag::None => ()
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
