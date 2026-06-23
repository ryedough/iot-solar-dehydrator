#![no_std]
#![no_main]

use cortex_m::asm::nop;
use embassy_executor::{Spawner, main, task};
use embassy_stm32::{bind_interrupts, i2c::Config, time::Hertz};
use defmt_rtt as _;
use defmt::*;
use defmt_rtt as _;
use embassy_time::Duration;
use panic_probe as _;

use crate::animation::{Animations, LogoAnimation, play_animation};

mod ssd1315;
mod animation;

bind_interrupts!(struct Irqs {
    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<embassy_stm32::peripherals::I2C1>;
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<embassy_stm32::peripherals::I2C1>;
    DMA1_CHANNEL6 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH6>;
    DMA1_CHANNEL7 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH7>;
});

#[main]
async fn main(spawner : Spawner) -> ! {
    let p = embassy_stm32::init(Default::default());

    let mut cfg = embassy_stm32::i2c::Config::default();
    cfg.frequency = Hertz(400_000);
    cfg.gpio_speed =embassy_stm32::gpio::Speed::VeryHigh;

    let i2c = embassy_stm32::i2c::I2c::new(p.I2C1, p.PB6, p.PB7, p.DMA1_CH6, p.DMA1_CH7, Irqs, cfg);
    let mut display = ssd1315::SSD1315::new(i2c);
    display.init().await.unwrap();

    let mut logo_anim : [Animations; _] = [LogoAnimation::new()];
    play_animation(&mut display, &mut logo_anim, Duration::from_millis(50)).await.unwrap();

    loop{
        nop();
    }
}

