use embassy_stm32::{exti::ExtiInput, mode::Async};

#[derive(PartialEq)]
pub enum Direction {
    Clockwise,
    CounterClockwise,
}

pub struct RotaryEncoder {
    a: ExtiInput<'static, Async>,
    b: ExtiInput<'static, Async>,
    last_state: u8,
}

impl RotaryEncoder {
    pub fn new(a: ExtiInput<'static, Async>, b: ExtiInput<'static, Async>) -> Self {
        let state =
            ((a.is_high() as u8) << 1) | (b.is_high() as u8);

        Self {
            a,
            b,
            last_state: state,
        }
    }

    pub async fn wait_direction(&mut self) -> Direction {
        loop {
            embassy_futures::select::select(
                self.a.wait_for_any_edge(),
                self.b.wait_for_any_edge(),
            )
            .await;

            let new_state =
                ((self.a.is_high() as u8) << 1) | (self.b.is_high() as u8);

            let transition = (self.last_state << 2) | new_state;

            self.last_state = new_state;

            match transition {
                0b0001
                | 0b0111
                | 0b1110
                | 0b1000 => return Direction::Clockwise,

                0b0010
                | 0b1011
                | 0b1101
                | 0b0100 => return Direction::CounterClockwise,

                _ => {}
            }
        }
    }
}
