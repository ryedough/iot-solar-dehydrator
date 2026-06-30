use embassy_time::Duration;
use embedded_graphics::image::ImageRaw;
use embedded_graphics::{pixelcolor::BinaryColor};

pub trait FlushableDisplay : embedded_graphics::draw_target::DrawTarget<Color=BinaryColor, Error : i2c::Error> {
    async fn flush(&mut self) -> Result<(), Self::Error>;
}

use embedded_graphics::{image::Image, prelude::*, primitives::{PrimitiveStyle, Rectangle, StyledDrawable}};
use embedded_hal_async::i2c;

use crate::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

// These const are defined in Ticks (not in millisecond)
// Tick are defined by the animate function
const LOGO_SWIPE_BOX_H : usize = 8;
const LOGO_SWIPE_BOX_SPEED : u8 = 12;
const LOGO_SWIPE_BOX_STAGGER : u8 = 1;
const LOGO_SHOW : u32 = 15;
pub struct LogoAnimation {
    swipe : [(u8, u8); 64 / LOGO_SWIPE_BOX_H],
    swipe_box_style : PrimitiveStyle<BinaryColor>,
    swipe_clear_style : PrimitiveStyle<BinaryColor>,
    ryedough_logo : ImageRaw<'static,BinaryColor>,
    swipe_full_at_tick : Option<u32>,
    tick_n : u32,
}

impl LogoAnimation {
    pub fn new()->Self{
        let ryedough_logo = include_bytes!("../assets/ryedough.bin");
        let ryedough_logo: ImageRaw<'_, BinaryColor> = ImageRaw::new(ryedough_logo, 64);
        Self {
            swipe : [(0,0); 64/LOGO_SWIPE_BOX_H],
            swipe_box_style : PrimitiveStyle::with_fill(BinaryColor::On),
            swipe_clear_style : PrimitiveStyle::with_fill(BinaryColor::Off),
            swipe_full_at_tick : None,
            ryedough_logo : ryedough_logo,
            tick_n : 0,
        }
    }
}

impl LogoAnimation {
    async fn tick<T: FlushableDisplay>(&mut self, display : &mut T) -> Result<(), T::Error> {

        // Draw sweep
        if self.swipe_full_at_tick.is_none() {
            for (i, (_, swipe_w)) in self.swipe.iter_mut().enumerate() {
                let stagger = i as u32 * LOGO_SWIPE_BOX_STAGGER as u32;
                if self.tick_n < stagger {
                    break;
                }
                if *swipe_w < DISPLAY_WIDTH {
                    *swipe_w += LOGO_SWIPE_BOX_SPEED;
                }

                let r = Rectangle::new(Point::new( 0, i as i32 * LOGO_SWIPE_BOX_H as i32),
                Size::new(*swipe_w as u32, LOGO_SWIPE_BOX_H as u32));
                r.draw_styled(&self.swipe_box_style, display)?;
            }
        }

        // Render logo
        let ryedough_logo = &self.ryedough_logo;
        let ryedough_logo = Image::new(
            ryedough_logo,
            Point::new(
                DISPLAY_WIDTH as i32 / 2 - (ryedough_logo.bounding_box().size.width as i32 /2),
                DISPLAY_HEIGHT as i32 / 2 - (ryedough_logo.bounding_box().size.height as i32/2),
        ));

        ryedough_logo.draw(display).unwrap();

        // Sweep clear (only after full draw sweep has achieved)
        if let Some(full_at) = self.swipe_full_at_tick {
            for (i, (swipe_w, _)) in self.swipe.iter_mut().enumerate() {
                let stagger = i as u32 * LOGO_SWIPE_BOX_STAGGER as u32;
                if self.tick_n > full_at + LOGO_SHOW + stagger {
                    if *swipe_w < 128 {
                        *swipe_w += LOGO_SWIPE_BOX_SPEED;
                    }
                }
                let r = Rectangle::new(Point::new(0 as i32, i as i32 * LOGO_SWIPE_BOX_H as i32),
                Size::new(*swipe_w as u32, LOGO_SWIPE_BOX_H as u32));
                r.draw_styled(&self.swipe_clear_style, display)?;
            }
        } else {
            if let Some((_, width)) = self.swipe.last().copied()
            && width >= 128{
                let _ = self.swipe_full_at_tick.insert(self.tick_n);
            }
        }

        self.tick_n += 1;
        display.flush().await
    }


    pub async fn animate<T: FlushableDisplay>(&mut self, display : &mut T, tick_duration : Duration)->Result<(), T::Error> {
        while !self.is_done(){
            let now = embassy_time::Instant::now();

            if self.is_done() { continue; }
            self.tick(display).await?;

            let diff = now.elapsed();
            if tick_duration > diff {
                let tick_duration = tick_duration - diff;
                embassy_time::Timer::after(tick_duration).await
            }
        }
        Ok(())
    }

    fn is_done(&self)->bool {
        if let Some((clear_w, draw_w)) = self.swipe.last().copied()
        && clear_w >= 128 && draw_w >= 128{
            true
        }else {
            false
        }
    }
}
