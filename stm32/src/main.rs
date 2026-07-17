#![no_std]
#![no_main]

use defmt::{error, info};
use defmt_rtt as _;
use defmt_rtt as _;
use embassy_executor::{Spawner, task};
use embassy_futures::yield_now;
use embassy_stm32::{
    bind_interrupts,
    exti::ExtiInput,
    gpio::{AfioRemap, Output},
    mode::Async,
    spi::Spi,
    time::Hertz,
    timer::simple_pwm::{PwmPin, SimplePwm},
};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, channel, watch::Watch};
use embassy_time::{Duration, Timer};
use embedded_graphics::{pixelcolor::BinaryColor, primitives::PrimitiveStyle};
use panic_probe as _;

use crate::{
    at24c08::AT24C08,
    channeled_signal::{ChanneledSignal, SignalSender},
    esp12f::{ESP12F, task::ESP12FExecuteCommandTaskMaker},
    menu::render_menu_task,
    rotary_encoder::listen_rotary_encoder_task,
    sht31::{SHT31Reading, read_sht_task},
};

mod animation;
mod at24c08;
mod channeled_signal;
mod esp12f;
mod menu;
mod rotary_encoder;
mod sht31;
mod ssd1315;

bind_interrupts!(struct Irqs {
    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<embassy_stm32::peripherals::I2C1>;
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<embassy_stm32::peripherals::I2C1>;
    DMA1_CHANNEL2 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH2>;
    DMA1_CHANNEL3 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH3>;
    DMA1_CHANNEL6 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH6>;
    DMA1_CHANNEL7 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH7>;
    EXTI0 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI0>;
    EXTI1 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI1>;
    EXTI2 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI2>;
    EXTI3 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI3>;
    EXTI4 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI4>;
});

#[derive(Clone, Copy)]
enum InputEvt {
    CounterClockwise,
    Clockwise,
    Enter,
}

const DISPLAY_WIDTH: u8 = 128;
const DISPLAY_HEIGHT: u8 = 64;

const PRIMITIVE_STYLE_ON: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_fill(BinaryColor::On);
const PRIMITIVE_STYLE_OFF: PrimitiveStyle<BinaryColor> =
    PrimitiveStyle::with_fill(BinaryColor::Off);
const PRIMITIVE_STYLE_BORDER_ONLY: PrimitiveStyle<BinaryColor> =
    PrimitiveStyle::with_stroke(BinaryColor::On, 1);

type SharedI2c = embassy_sync::mutex::Mutex<
    embassy_sync::blocking_mutex::raw::ThreadModeRawMutex,
    Option<
        embassy_stm32::i2c::I2c<'static, embassy_stm32::mode::Async, embassy_stm32::i2c::Master>,
    >,
>;
static I2C: SharedI2c = embassy_sync::mutex::Mutex::new(Option::None);
static INPUT_CH: ChanneledSignal<InputEvt> = ChanneledSignal::new();
static CALIBRATION_CH: ChanneledSignal<SHT31Reading> = ChanneledSignal::new();
static CLIMATE_WATCH: Watch<ThreadModeRawMutex, SHT31Reading, 2> = Watch::new();

static ESP12_TM: ESP12FExecuteCommandTaskMaker = ESP12FExecuteCommandTaskMaker::new(
    [channel::Channel::new(), channel::Channel::new()],
    [channel::Channel::new(), channel::Channel::new()],
);

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    use animation::*;
    let p = embassy_stm32::init(Default::default());

    // init i2c
    let mut cfg = embassy_stm32::i2c::Config::default();
    cfg.frequency = embassy_stm32::time::Hertz(400_000);
    let i2c = embassy_stm32::i2c::I2c::new(p.I2C1, p.PB6, p.PB7, p.DMA1_CH6, p.DMA1_CH7, Irqs, cfg);
    let _ = I2C.lock().await.insert(i2c);

    // init display
    let mut display = loop {
        let display = ssd1315::SSD1315::init().await;
        match display {
            Ok(d) => break d,
            Err(_) => {
                error!("Display not found, retrying...");
                Timer::after_secs(5).await;
            }
        };
    };

    // init esp12f
    // let mut spi_config: embassy_stm32::spi::Config = Default::default();
    // spi_config.mode = embassy_stm32::spi::MODE_0;
    // spi_config.frequency = Hertz(1_000_000);
    // let spi = Spi::new(
    //     p.SPI1, p.PA5, p.PA7, p.PA6, p.DMA1_CH3, p.DMA1_CH2, Irqs, spi_config,
    // );
    // let spi_cs = Output::new(
    //     p.PA12,
    //     embassy_stm32::gpio::Level::Low,
    //     embassy_stm32::gpio::Speed::VeryHigh,
    // );
    // let esp8266_handshake_pin =
    //     ExtiInput::new(p.PA3, p.EXTI3, embassy_stm32::gpio::Pull::None, Irqs);
    // let esp8266_reset_pin = Output::new(
    //     p.PA11,
    //     embassy_stm32::gpio::Level::Low,
    //     embassy_stm32::gpio::Speed::Medium,
    // );
    // let esp12f = ESP12F::new(spi, esp8266_handshake_pin, spi_cs, esp8266_reset_pin);
    // let [esp_ch1, esp_ch2] = ESP12_TM.create_esp12f_execute_command_task(&spawner, esp12f);

    // init button
    let pin_a = ExtiInput::new(p.PA1, p.EXTI1, embassy_stm32::gpio::Pull::None, Irqs);
    let pin_b = ExtiInput::new(p.PA2, p.EXTI2, embassy_stm32::gpio::Pull::None, Irqs);
    let enter_btn = ExtiInput::new(p.PA0, p.EXTI0, embassy_stm32::gpio::Pull::Down, Irqs);

    spawner.spawn(listen_rotary_encoder_task(INPUT_CH.sender(), pin_a, pin_b).unwrap());
    spawner.spawn(listen_input(INPUT_CH.sender(), enter_btn, InputEvt::Enter).unwrap());

    // load setting and animate splash screen concurrently
    let mut logo_anim = LogoAnimation::new();
    let ((eeprom, settings), logo_anim) = embassy_futures::join::join(
        AT24C08::new_and_load_settings(),
        logo_anim.animate(&mut display, Duration::from_millis(50)),
    )
    .await;
    logo_anim.unwrap();
    let calibration_ss = CALIBRATION_CH.sender();
    calibration_ss.send(settings.calibration.clone());

    //init pwm
    let pwm_pin: PwmPin<'_, _, _, AfioRemap<0>> =
        PwmPin::new(p.PA8, embassy_stm32::gpio::OutputType::PushPull);
    let mut pwm = SimplePwm::new(
        p.TIM1,
        Some(pwm_pin),
        None,
        None,
        None,
        Hertz(20000),
        Default::default(),
    );
    pwm.ch1().enable();
    pwm.ch1()
        .set_duty_cycle_percent(settings.fan_speed.as_percent());

    // init task
    spawner.spawn(read_sht_task(CALIBRATION_CH.receiver(), CLIMATE_WATCH.sender()).unwrap());
    spawner.spawn(
        render_menu_task(
            display,
            eeprom,
            pwm,
            INPUT_CH.receiver(),
            CLIMATE_WATCH.receiver().unwrap(),
            calibration_ss,
            settings.calibration,
            settings.fan_speed,
        )
        .unwrap(),
    );
    loop {
        yield_now().await;
    }
}

#[task]
async fn listen_input(
    input_ss: SignalSender<InputEvt>,
    mut btn: ExtiInput<'static, Async>,
    value: InputEvt,
) {
    loop {
        btn.wait_for_rising_edge().await;
        input_ss.send(value);
        Timer::after_millis(200).await;
    }
}
