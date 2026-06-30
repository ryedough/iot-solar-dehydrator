use core::{fmt::Write, ops::{Add, AddAssign, Sub, SubAssign}};
use embassy_time::{Duration};
use embedded_graphics::{
    Drawable,
    image::{Image, ImageRawBE},
    mono_font::MonoTextStyle,
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle, StyledDrawable},
    text::Text,
};

use crate::{
    DISPLAY_HEIGHT, DISPLAY_WIDTH, InputEvt, animation::FlushableDisplay, menu::{BareMenu, Lerp, Menu}, sht31::SHT31Reading
};
struct MenuItem {
    name: &'static str,
    logo: ImageRawBE<'static, BinaryColor>,
}

struct MiniLogo {
    temp: ImageRawBE<'static, BinaryColor>,
    humid: ImageRawBE<'static, BinaryColor>,
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum SideMenu {
    Fan = 0,
    Wifi = 1,
    Sensor = 2,
}

impl SubAssign<u8> for SideMenu {
    fn sub_assign(&mut self, rhs: u8) {
        *self = *self - rhs;
    }
}
impl AddAssign<u8> for SideMenu {
    fn add_assign(&mut self, rhs: u8) {
        *self = *self + rhs;
    }
}
impl Add<u8> for SideMenu {
    type Output = SideMenu;
    fn add(self, rhs: u8) -> Self::Output {
        let mut v = self as u8 + rhs;
        if v > Self::LEN as u8 -1 {v=Self::LEN as u8 -1};
        v.into()
    }
}
impl Sub<u8> for SideMenu {
    type Output = SideMenu;
    fn sub(self, rhs: u8) -> Self::Output {
        let mut v = self as u8 - rhs;
        if v < 0 {v=0};
        v.into()
    }
}

impl From<u8> for SideMenu {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Fan,
            1 => Self::Wifi,
            2 => Self::Sensor,
            _ => panic!("Invalid value"),
        }
    }
}

impl SideMenu {
    const LEN : usize = 3;
}

const MENU_ITEM_GAP: u32 = 3;
pub struct MainMenu {
    side_menu_items: [MenuItem; SideMenu::LEN],
    selected_side_menu: SideMenu,
    mini_logo: MiniLogo,
    side_menu_lerp: Option<Lerp>,
    changed: bool,
    climate: Option<SHT31Reading>,
    text_char : MonoTextStyle<'static, BinaryColor>,
}

pub enum OnInputFlag {
    ToSensorMenu,
    None,
}

const SIDE_MENU_LERP_DURATION: Duration = Duration::from_millis(200);
impl MainMenu {
    pub fn new() -> Self {
        let side_menu_items: [MenuItem; SideMenu::LEN] = [
            MenuItem {
                name: "Fan",
                logo: ImageRawBE::new(include_bytes!("../../assets/fan.bin"), 25),
            },
            MenuItem {
                name: "WiFi",
                logo: ImageRawBE::new(include_bytes!("../../assets/wifi.bin"), 25),
            },
            MenuItem {
                name: "Sensor",
                logo: ImageRawBE::new(include_bytes!("../../assets/sensor.bin"), 25),
            },
        ];

        let mini_logo = MiniLogo {
            temp: ImageRawBE::new(include_bytes!("../../assets/mini-temp.bin"), 5),
            humid: ImageRawBE::new(include_bytes!("../../assets/mini-humid.bin"), 5),
        };

        return Self {
            side_menu_items,
            selected_side_menu: 0.into(),
            mini_logo,
            side_menu_lerp: None,
            changed: true,
            climate: None,
            text_char: MonoTextStyle::new(
                &embedded_graphics::mono_font::ascii::FONT_5X8,
                BinaryColor::On,
            )
        };
    }
    pub async fn set_climate(&mut self, climate: SHT31Reading) {
        let _ = self.climate.insert(climate);
        self.changed = true;
    }
    async fn render(&self, display: &mut impl FlushableDisplay) {
        display.clear(BinaryColor::Off).unwrap();

        let mini_temp_logo = Image::new(&self.mini_logo.temp, Point::new(0, 2));
        let mini_humid_logo = Image::new(&self.mini_logo.humid, Point::new(0, 15));

        mini_temp_logo.draw(display).unwrap();
        mini_humid_logo.draw(display).unwrap();
        match &self.climate {
            Some(climate) => {
                {
                    let mut temp: heapless::String<10> = heapless::String::new();
                    write!(temp, "{:.1}C", climate.temp).unwrap();
                    Text::new(&temp, Point::new(9, 10), self.text_char)
                        .draw(display)
                        .unwrap();
                }
                {
                    let mut humid: heapless::String<10> = heapless::String::new();
                    write!(humid, "{:.1}%", climate.humid).unwrap();
                    Text::new(&humid, Point::new(9, 21), self.text_char)
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
            let is_selected = self.selected_side_menu as usize == i;

            let lerp = if let Some(lerp) = self.side_menu_lerp.as_ref() {
                lerp.get()
            } else {
                self.selected_side_menu as u8 as f32
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
            self.side_menu_items[self.selected_side_menu as usize].name,
            Point::new(title_rect_x + title_rect_w / 2, title_rect_y - 5),
            self.text_char,
            embedded_graphics::text::Alignment::Center,
        )
        .draw(display)
        .unwrap();

        display.flush().await.unwrap();
    }
    pub fn on_input(&mut self, evt: InputEvt) -> OnInputFlag {
        let old_selected_side_menu = self.selected_side_menu;
        match evt {
            InputEvt::Up => {
                if self.selected_side_menu > 0.into() {
                    self.selected_side_menu -= 1;
                    self.changed = true;
                }
            }
            InputEvt::Down => {
                if self.selected_side_menu < (SideMenu::LEN as u8-1).into() {
                    self.selected_side_menu += 1;
                    self.changed = true;
                }
            }
            InputEvt::Enter => {
                if self.selected_side_menu == SideMenu::Sensor {
                    return OnInputFlag::ToSensorMenu;
                }
            }
        }
        if self.changed {
            match self.side_menu_lerp.take() {
                Some(old_lerp) => {
                    let _ = self.side_menu_lerp.insert(Lerp::new(
                        old_lerp.get(),
                        self.selected_side_menu as u8 as f32,
                        SIDE_MENU_LERP_DURATION,
                    ));
                }
                None => {
                    let _ = self.side_menu_lerp.insert(Lerp::new(
                        old_selected_side_menu as u8 as f32,
                        self.selected_side_menu as u8 as f32,
                        SIDE_MENU_LERP_DURATION,
                    ));
                }
            }
        }
        OnInputFlag::None
    }
}

impl BareMenu for MainMenu {
    async fn tick(&mut self, display: &mut impl FlushableDisplay) {
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
}
