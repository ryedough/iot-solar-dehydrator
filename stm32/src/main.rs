#![no_std]
#![no_main]

use embassy_executor::{Spawner, main};
use embassy_stm32::{bind_interrupts, interrupt, exti};
use panic_halt as _;

bind_interrupts!(
    struct Irqs {
        EXTI0 => exti::InterruptHandler<interrupt::typelevel::EXTI0>;
    });

#[main]
async fn main(spawner : Spawner) {
    let p = embassy_stm32::init(Default::default());
    let mut down = exti::ExtiInput::new(p.PA0, p.EXTI0, embassy_stm32::gpio::Pull::None, Irqs);

    loop {
        down.wait_for_rising_edge().await;
    }
}
