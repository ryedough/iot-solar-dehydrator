mod logo_animation;
pub use logo_animation::LogoAnimation;

use core::{fmt::{Debug}};

use embassy_time::Duration;
use embedded_graphics::{pixelcolor::BinaryColor};

pub trait FlushableDisplay : embedded_graphics::draw_target::DrawTarget<Color=BinaryColor, Error : Debug> {
    async fn flush(&mut self) -> Result<(), Self::Error>;
}

trait CheckableAnimation {
    fn is_done(&self)->bool;
}

trait Animation<D : FlushableDisplay> : CheckableAnimation {
    async fn setup(&mut self, _display : &mut D) -> Result<(), D::Error>{
        Ok(())
    }
    async fn tick(&mut self, display : &mut D) -> Result<(), D::Error>;
}

pub enum Animations {
    Logo(LogoAnimation),
}

impl Animations {
    async fn tick<D: FlushableDisplay>(&mut self, display : &mut D) -> Result<(), D::Error>{
        match self {
            Self::Logo(animation) => {
                animation.tick(display).await
            },
        }
    }
    fn is_done(&self)->bool {
        match self {
            Self::Logo(animation) => animation.is_done(),
        }
    }
}

pub async fn animate<T : FlushableDisplay>(display : &mut T, animations : &mut [Animations], tick_duration : Duration)->Result<(), T::Error> {
    while !animations.last().unwrap().is_done(){
        let now = embassy_time::Instant::now();
        for animation in animations.iter_mut() {
            if animation.is_done() { continue; }
            animation.tick(display).await?;
        }

        let diff = now.elapsed();
        if tick_duration > diff {
            let tick_duration = tick_duration - diff;
            embassy_time::Timer::after(tick_duration).await
        }
    }
    Ok(())
}
