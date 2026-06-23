use defmt::info;
use embedded_graphics::{image::Image, pixelcolor::BinaryColor, prelude::*, primitives::{PrimitiveStyle, Rectangle, StyledDrawable}};
use tinybmp::Bmp;

use crate::animation::{Animation, Animations, FlushableDisplay, CheckableAnimation};

const LOGO_SWIPE_BOX_H : usize = 8;
const LOGO_SWIPE_BOX_SPEED : u8 = 10;
const LOGO_SWIPE_BOX_STAGGER : u8 = 3;
pub struct LogoAnimation {
    swipe_box_w : [u8; 64 / LOGO_SWIPE_BOX_H],
    swipe_box_style : PrimitiveStyle<BinaryColor>,
    ryedough_logo : Bmp<'static,BinaryColor>,
    tick_n : u32,
}

impl LogoAnimation {
    pub fn new()->Animations{
        let ryedough_logo = include_bytes!("../../assets/ryedough.bmp");
        let ryedough_logo: Bmp<'_, BinaryColor> = Bmp::from_slice(ryedough_logo).unwrap();
        Animations::Logo(Self {
            swipe_box_w : [0; 64/LOGO_SWIPE_BOX_H],
            swipe_box_style : PrimitiveStyle::with_fill(BinaryColor::On),
            ryedough_logo : ryedough_logo,
            tick_n : 0,
        })
    }
}

impl<D : FlushableDisplay> Animation<D> for LogoAnimation {
    async fn tick(&mut self, display : &mut D) -> Result<(), D::Error> {
        let display_w = display.bounding_box().size.width;
        let display_h = display.bounding_box().size.height;

        for (i, swipe_box_w) in self.swipe_box_w.iter_mut().enumerate() {
            let stagger = i as u32 * LOGO_SWIPE_BOX_STAGGER as u32;
            if self.tick_n < stagger {
                break;
            }
            let r = Rectangle::new(Point::new(0, i as i32 * LOGO_SWIPE_BOX_H as i32), 
                Size::new(*swipe_box_w as u32, LOGO_SWIPE_BOX_H as u32));
            r.draw_styled(&self.swipe_box_style, display)?;
            if (*swipe_box_w as u32) < display_w {
                *swipe_box_w += LOGO_SWIPE_BOX_SPEED;
            }
        }

        let ryedough_logo = &self.ryedough_logo;
        let ryedough_logo = Image::new(
            ryedough_logo,
            Point::new(
                display_w as i32 / 2 - (ryedough_logo.bounding_box().size.width as i32 /2),
                display_h as i32 / 2 - (ryedough_logo.bounding_box().size.height as i32/2),
        ));
        ryedough_logo.draw(display).unwrap();

        self.tick_n += 1;
        display.flush().await
    }
}

impl CheckableAnimation for LogoAnimation {
    fn is_done(&self)->bool {
        // info!("last_w : {:?}", self.swipe_box_w.last().copied().unwrap_or(0));
        self.swipe_box_w.last().copied().unwrap_or(0) >= 128
    }
}
