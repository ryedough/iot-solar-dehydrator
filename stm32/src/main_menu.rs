use core::fmt::Write;

use defmt::info;
use embassy_time::{Duration, Instant};
use embedded_graphics::{
    Drawable,
    image::Image,
    mono_font::MonoTextStyle,
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle, StyledDrawable},
    text::Text,
};
use tinybmp::Bmp;

use crate::{
    DISPLAY_HEIGHT, DISPLAY_WIDTH, InputDirection, animation::FlushableDisplay, sht31::SHT31Reading,
};

struct MenuItem {
    name: &'static str,
    logo: Bmp<'static, BinaryColor>,
}

struct MiniLogo {
    temp: Bmp<'static, BinaryColor>,
    humid: Bmp<'static, BinaryColor>,
}

const MENU_ITEM_LEN: usize = 3;
const MENU_ITEM_GAP: u32 = 3;
pub struct MainMenu {
    side_menu_items: [MenuItem; MENU_ITEM_LEN],
    selected_side_menu_id: u8,
    mini_logo: MiniLogo,
    side_menu_lerp: Option<Lerp>,
    changed: bool,
    climate: Option<SHT31Reading>,
}

struct Lerp {
    a: f32,
    b: f32,
    start: Instant,
    duration: Duration,
}

impl Lerp {
    pub fn new(a: f32, b: f32, duration: Duration) -> Self {
        Self {
            a,
            b,
            start: Instant::now(),
            duration,
        }
    }
    pub fn get(&self) -> f32 {
        let mut t = self.start.elapsed().as_millis() as f32 / self.duration.as_millis() as f32;
        if t > 1. {
            t = 1.
        };
        self.b * t + self.a * (1. - t)
    }
    pub fn is_done(&self) -> bool {
        self.start.elapsed() > self.duration
    }
}

const LERP_DURATION: Duration = Duration::from_millis(200);
impl MainMenu {
    pub fn new() -> Self {
        let side_menu_items: [MenuItem; MENU_ITEM_LEN] = [
            MenuItem {
                name: "Fan",
                logo: Bmp::from_slice(include_bytes!("../assets/fan.bmp")).unwrap(),
            },
            MenuItem {
                name: "WiFi",
                logo: Bmp::from_slice(include_bytes!("../assets/wifi.bmp")).unwrap(),
            },
            MenuItem {
                name: "Sensor",
                logo: Bmp::from_slice(include_bytes!("../assets/sensor.bmp")).unwrap(),
            },
        ];

        let mini_logo = MiniLogo {
            temp: Bmp::from_slice(include_bytes!("../assets/mini-temp.bmp")).unwrap(),
            humid: Bmp::from_slice(include_bytes!("../assets/mini-humid.bmp")).unwrap(),
        };

        return Self {
            side_menu_items,
            selected_side_menu_id: 0,
            mini_logo,
            side_menu_lerp: None,
            changed: true,
            climate: None,
        };
    }
    pub async fn set_climate(&mut self, climate: SHT31Reading) {
        let _ = self.climate.insert(climate);
        self.changed = true;
    }
    pub fn menu_set(&mut self, dir: InputDirection) {
        let old_selected_side_menu_id = self.selected_side_menu_id;
        match dir {
            InputDirection::Up => {
                if self.selected_side_menu_id > 0 {
                    self.selected_side_menu_id -= 1;
                    self.changed = true;
                }
            }
            InputDirection::Down => {
                if self.selected_side_menu_id < self.side_menu_items.len() as u8 - 1 {
                    self.selected_side_menu_id += 1;
                    self.changed = true;
                }
            }
        }
        if self.changed {
            match self.side_menu_lerp.take() {
                Some(old_lerp) => {
                    let _ = self.side_menu_lerp.insert(Lerp::new(
                        old_lerp.get(),
                        self.selected_side_menu_id.into(),
                        LERP_DURATION,
                    ));
                }
                None => {
                    let _ = self.side_menu_lerp.insert(Lerp::new(
                        old_selected_side_menu_id.into(),
                        self.selected_side_menu_id.into(),
                        LERP_DURATION,
                    ));
                }
            }
        }
    }
    pub async fn tick(&mut self, display: &mut impl FlushableDisplay) {
        if !self.changed && self.side_menu_lerp.is_none() {
            return;
        }

        self.changed = false;
        self.render(display).await;

        if let Some(lerp) = self.side_menu_lerp.take() {
            if !lerp.is_done() {
                let _ = self.side_menu_lerp.insert(lerp);
            }
            self.changed = true;// render one last time
        }
    }
    async fn render(&self, display: &mut impl FlushableDisplay) {
        display.clear(BinaryColor::Off).unwrap();
        let character_style = MonoTextStyle::new(
            &embedded_graphics::mono_font::ascii::FONT_5X8,
            BinaryColor::On,
        );

        let mini_temp_logo = Image::new(&self.mini_logo.temp, Point::new(0, 2));
        let mini_humid_logo = Image::new(&self.mini_logo.humid, Point::new(0, 15));

        mini_temp_logo.draw(display).unwrap();
        mini_humid_logo.draw(display).unwrap();
        match &self.climate {
            Some(climate) => {
                {
                    let mut temp: heapless::String<10> = heapless::String::new();
                    write!(temp, "{:.1}C", climate.temp).unwrap();
                    Text::new(&temp, Point::new(9, 10), character_style)
                        .draw(display)
                        .unwrap();
                }
                {
                    let mut humid: heapless::String<10> = heapless::String::new();
                    write!(humid, "{:.1}%", climate.humid).unwrap();
                    Text::new(&humid, Point::new(9, 21), character_style)
                        .draw(display)
                        .unwrap();
                }
            }
            None => {}
        }

        let padding = 4;
        let mut cur_y = DISPLAY_HEIGHT as i32 / 2;
        for (i, item) in self.side_menu_items.iter().enumerate() {
            let item_height = item.logo.size().height as i32;
            let item_width = item.logo.size().width;
            let is_selected = self.selected_side_menu_id as usize == i;

            let lerp = if let Some(lerp) = self.side_menu_lerp.as_ref() {
                lerp.get()
            } else {
                self.selected_side_menu_id as f32
            };

            let logo_position = Point::new(
                DISPLAY_WIDTH as i32
                    - item_width as i32
                    - padding
                    - if is_selected { 5 } else { 0 },
                (cur_y as f32
                    - (item_height as f32 / 2.)
                    - (lerp * item_height as f32 + padding as f32)) as i32,
            );
            let logo = Image::new(&item.logo, logo_position);
            cur_y += logo.bounding_box().size.height as i32 + MENU_ITEM_GAP as i32;
            logo.draw(display).unwrap();
        }

        let style = PrimitiveStyle::with_fill(BinaryColor::On);

        let logo_width = self.side_menu_items[0].logo.size().width as i32;
        let title_rect_w = 30;
        let title_rect_h = 3;
        let title_rect_x = DISPLAY_WIDTH as i32 - logo_width - padding * 2 - title_rect_w - 5;
        let title_rect_y = DISPLAY_HEIGHT as i32 - title_rect_h;
        Rectangle::new(
            Point::new(title_rect_x, title_rect_y),
            Size::new(title_rect_w as u32, title_rect_h as u32),
        )
        .draw_styled(&style, display)
        .unwrap();

        Text::with_alignment(
            self.side_menu_items[self.selected_side_menu_id as usize].name,
            Point::new(title_rect_x + title_rect_w / 2, title_rect_y - 5),
            character_style,
            embedded_graphics::text::Alignment::Center,
        )
        .draw(display)
        .unwrap();

        display.flush().await.unwrap();
    }
}
