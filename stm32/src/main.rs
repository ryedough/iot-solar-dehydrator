#![no_std]
#![no_main]

use cortex_m::asm::nop;
use embassy_executor::{Spawner, main, task};
use embassy_stm32::{bind_interrupts};
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*, primitives::{Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StyledDrawable}};
use defmt_rtt as _;
use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

mod ssd1315;

bind_interrupts!(struct Irqs {
    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<embassy_stm32::peripherals::I2C1>;
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<embassy_stm32::peripherals::I2C1>;
    DMA1_CHANNEL6 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH6>;
    DMA1_CHANNEL7 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH7>;
});

#[main]
async fn main(spawner : Spawner) -> ! {
    let p = embassy_stm32::init(Default::default());

    let i2c = embassy_stm32::i2c::I2c::new(p.I2C1, p.PB6, p.PB7, p.DMA1_CH6, p.DMA1_CH7, Irqs, Default::default());
    let mut ssd1315 = ssd1315::SSD1315::new(i2c);
    ssd1315.init().await.unwrap();
    for y in 0..11 {
        for x in 0..11 {
            ssd1315.set_pixel(x, y, true);
        }
    }
    ssd1315.flush().await.unwrap();
    loop{
        nop();
    }
}

