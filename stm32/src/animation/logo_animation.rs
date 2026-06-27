use embedded_graphics::{image::Image, pixelcolor::BinaryColor, prelude::*, primitives::{PrimitiveStyle, Rectangle, StyledDrawable}};
use tinybmp::Bmp;

use crate::animation::{Animation, Animations, FlushableDisplay, CheckableAnimation};

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
    ryedough_logo : Bmp<'static,BinaryColor>,
    swipe_full_at_tick : Option<u32>,
    tick_n : u32,
}

impl LogoAnimation {
    pub fn new()->Animations{
        let ryedough_logo = include_bytes!("../../assets/ryedough.bmp");
        let ryedough_logo: Bmp<'_, BinaryColor> = Bmp::from_slice(ryedough_logo).unwrap();
        Animations::Logo(Self {
            swipe : [(0,0); 64/LOGO_SWIPE_BOX_H],
            swipe_box_style : PrimitiveStyle::with_fill(BinaryColor::On),
            swipe_clear_style : PrimitiveStyle::with_fill(BinaryColor::Off),
            swipe_full_at_tick : None,
            ryedough_logo : ryedough_logo,
            tick_n : 0,
        })
    }
}

impl<D : FlushableDisplay> Animation<D> for LogoAnimation {
    async fn tick(&mut self, display : &mut D) -> Result<(), D::Error> {
        let display_w = display.bounding_box().size.width;
        let display_h = display.bounding_box().size.height;

        // Draw sweep
        if self.swipe_full_at_tick.is_none() {
            for (i, (_, swipe_w)) in self.swipe.iter_mut().enumerate() {
                let stagger = i as u32 * LOGO_SWIPE_BOX_STAGGER as u32;
                if self.tick_n < stagger {
                    break;
                }
                if (*swipe_w as u32) < display_w {
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
                display_w as i32 / 2 - (ryedough_logo.bounding_box().size.width as i32 /2),
                display_h as i32 / 2 - (ryedough_logo.bounding_box().size.height as i32/2),
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
}

impl CheckableAnimation for LogoAnimation {
    fn is_done(&self)->bool {
        if let Some((clear_w, draw_w)) = self.swipe.last().copied()
        && clear_w >= 128 && draw_w >= 128{
            true
        }else {
            false
        }
    }
}
