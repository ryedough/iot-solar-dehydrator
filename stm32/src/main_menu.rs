use core::fmt::{self, Write};

use embedded_graphics::{Drawable, image::Image, mono_font::{MonoTextStyle, ascii::FONT_6X12}, pixelcolor::BinaryColor, prelude::*, text::{Text, TextStyleBuilder}};
use tinybmp::Bmp;

use crate::{animation::FlushableDisplay, sht31::SHT31Reading};

struct Position {
    x : f32,
    y : f32,
}
impl Default for Position {
    fn default() -> Self {
        Self{
            x : 0.,
            y : 0.,
        }
    }
}

struct MenuItem {
    name : &'static str,
    logo : Bmp<'static, BinaryColor>,
}

struct MiniLogo {
    temp : Bmp<'static, BinaryColor>,
    humid : Bmp<'static, BinaryColor>,
}

const MENU_ITEM_LEN : usize = 3;
const MENU_ITEM_GAP : u32 = 10;
pub struct MainMenu{
    side_menu_items : [MenuItem; MENU_ITEM_LEN],
    selected_side_menu_id : u8,
    total_side_menu_height : u32,
    mini_logo : MiniLogo,
    lerp : Option<&'static dyn FnMut(u64)>,
    changed : bool,
    climate : Option<SHT31Reading>,
}

impl MainMenu{
    pub fn new() -> Self {
        let side_menu_items : [MenuItem; MENU_ITEM_LEN] = [
            MenuItem {
                name : "Fan",
                logo : Bmp::from_slice(include_bytes!("../assets/fan.bmp")).unwrap(),
            },
            MenuItem {
                name : "WiFi",
                logo : Bmp::from_slice(include_bytes!("../assets/wifi.bmp")).unwrap(),
            },
            MenuItem {
                name : "Sensor",
                logo : Bmp::from_slice(include_bytes!("../assets/sensor.bmp")).unwrap(),
            }
        ];

        let mut total_side_menu_height = MENU_ITEM_GAP * (MENU_ITEM_LEN as u32 - 1);
        for item in side_menu_items.iter() {
            total_side_menu_height += item.logo.size().height;
        }

        let mini_logo = MiniLogo {
            temp : Bmp::from_slice(include_bytes!("../assets/mini-temp.bmp")).unwrap(),
            humid : Bmp::from_slice(include_bytes!("../assets/mini-humid.bmp")).unwrap(),
        };

        return Self {
            side_menu_items,
            selected_side_menu_id: 0,
            total_side_menu_height,
            mini_logo,
            lerp : None,
            changed : true,
            climate : None,
        }
    }
    pub async fn set_climate(&mut self, climate : SHT31Reading) {
        let _ = self.climate.insert(climate);
        self.changed = true;
    }
    pub async fn tick(&mut self, display: &mut impl FlushableDisplay) {
        if !self.changed {
            return;
        }

        self.changed = false;
        self.render(display).await;
    }
    async fn render(&self, display: &mut impl FlushableDisplay){
        display.clear(BinaryColor::Off);
        let temp_logo = Image::new(&self.mini_logo.temp, Point::new(0, 2));
        let humid_logo = Image::new(&self.mini_logo.humid, Point::new(0, 15));

        temp_logo.draw(display).unwrap();
        humid_logo.draw(display).unwrap();
        match &self.climate {
            Some(climate) => {
                let character_style = MonoTextStyle::new(&FONT_6X12, BinaryColor::On);
                {
                    let mut temp: heapless::String<10> = heapless::String::new();
                    write!(temp, "{:.1}C", climate.temp).unwrap();
                    Text::new( &temp, Point::new(11, 11), character_style)
                        .draw(display).unwrap();
                } 
                {
                    let mut humid: heapless::String<10> = heapless::String::new();
                    write!(humid, "{:.1}%", climate.humid).unwrap();
                    Text::new( &humid, Point::new(11, 21), character_style)
                        .draw(display).unwrap();
                }
            },
            None => {},
        }

        display.flush().await.unwrap();
    }
}


