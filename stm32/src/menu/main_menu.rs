use core::{fmt::Write, ops::{Add, AddAssign, Sub, SubAssign}};
use embassy_time::{Duration};
use embedded_graphics::{
    Drawable,
    image::{Image, ImageRaw},
    mono_font::MonoTextStyle,
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle, StyledDrawable},
    text::Text,
};

use crate::{
    DISPLAY_HEIGHT, DISPLAY_WIDTH, InputEvt, PRIMITIVE_STYLE_ON, animation::FlushableDisplay, menu::{BareMenu, Lerp, Menu}, sht31::SHT31Reading
};
struct MenuItem {
    name: &'static str,
    logo: ImageRaw<'static, BinaryColor>,
}

struct MiniLogo {
    temp: ImageRaw<'static, BinaryColor>,
    humid: ImageRaw<'static, BinaryColor>,
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum Selection {
    Fan = 0,
    Wifi = 1,
    Sensor = 2,
}

impl SubAssign<u8> for Selection {
    fn sub_assign(&mut self, rhs: u8) {
        *self = *self - rhs;
    }
}
impl AddAssign<u8> for Selection {
    fn add_assign(&mut self, rhs: u8) {
        *self = *self + rhs;
    }
}
impl Add<u8> for Selection {
    type Output = Selection;
    fn add(self, rhs: u8) -> Self::Output {
        let mut v = self as u8 + rhs;
        if v > Self::LEN as u8 -1 {v=Self::LEN as u8 -1};
        v.into()
    }
}
impl Sub<u8> for Selection {
    type Output = Selection;
    fn sub(self, rhs: u8) -> Self::Output {
        let v = if self as u8 > 0 {self as u8 - rhs} else {0};
        v.into()
    }
}

impl From<u8> for Selection {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Fan,
            1 => Self::Wifi,
            2 => Self::Sensor,
            _ => panic!("Invalid value"),
        }
    }
}

impl Selection {
    const LEN : usize = 3;
}

const MENU_ITEM_GAP: u32 = 3;
pub struct MainMenu {
    selection: [MenuItem; Selection::LEN],
    selected: Selection,
    mini_logo: MiniLogo,
    side_menu_lerp: Option<Lerp>,
    changed: bool,
    climate: Option<SHT31Reading>,
    text_char : MonoTextStyle<'static, BinaryColor>,
    label_char : MonoTextStyle<'static, BinaryColor>
}

pub enum OnInputFlag {
    ToSensorMenu,
    ToFanMenu,
    None,
}

const SIDE_MENU_LERP_DURATION: Duration = Duration::from_millis(200);
impl MainMenu {
    pub fn new(selected : Option<Selection>) -> Self {
        let side_menu_items: [MenuItem; Selection::LEN] = [
            MenuItem {
                name: "Fan",
                logo: ImageRaw::new(include_bytes!("../../assets/fan.bin"), 25),
            },
            MenuItem {
                name: "WiFi",
                logo: ImageRaw::new(include_bytes!("../../assets/wifi.bin"), 25),
            },
            MenuItem {
                name: "Sensor",
                logo: ImageRaw::new(include_bytes!("../../assets/sensor.bin"), 25),
            },
        ];

        let mini_logo = MiniLogo {
            temp: ImageRaw::new(include_bytes!("../../assets/mini-temp.bin"), 5),
            humid: ImageRaw::new(include_bytes!("../../assets/mini-humid.bin"), 5),
        };

        return Self {
            selection: side_menu_items,
            selected: selected.unwrap_or(0.into()),
            mini_logo,
            side_menu_lerp: None,
            changed: true,
            climate: None,
            text_char: MonoTextStyle::new(
                &embedded_graphics::mono_font::ascii::FONT_5X8,
                BinaryColor::On,
            ),
            label_char : MonoTextStyle::new(
                &embedded_graphics::mono_font::ascii::FONT_6X12, BinaryColor::On)
        };
    }
    pub async fn set_climate(&mut self, climate: SHT31Reading) {
        let _ = self.climate.insert(climate);
        self.changed = true;
    }
    async fn render(&self, display: &mut impl FlushableDisplay) {
        display.clear(BinaryColor::Off).unwrap();

        // draw humid & temp info
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

        // draw side selection
        let padding = 4;
        let mut cur_y = DISPLAY_HEIGHT as i32 / 2;
        for (i, item) in self.selection.iter().enumerate() {
            let item_height = item.logo.size().height as i32;
            let item_width = item.logo.size().width;
            let is_selected = self.selected as usize == i;

            let lerp = if let Some(lerp) = self.side_menu_lerp.as_ref() {
                lerp.get()
            } else {
                self.selected as u8 as f32
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

        // display selection label
        let logo_width = self.selection[0].logo.size().width as i32;
        let title_rect_w = 34;
        let title_rect_h = 3;
        let title_rect_x = DISPLAY_WIDTH as i32 - logo_width - padding * 2 - title_rect_w - 7;
        let title_rect_y = DISPLAY_HEIGHT as i32 - title_rect_h;
        Rectangle::new(
            Point::new(title_rect_x, title_rect_y),
            Size::new(title_rect_w as u32, title_rect_h as u32),
        )
        .draw_styled(&PRIMITIVE_STYLE_ON, display)
        .unwrap();

        Text::with_alignment(
            self.selection[self.selected as usize].name,
            Point::new(title_rect_x + title_rect_w / 2, title_rect_y - 5),
            self.label_char,
            embedded_graphics::text::Alignment::Center,
        )
        .draw(display)
        .unwrap();

        display.flush().await.unwrap();
    }
}

impl BareMenu for MainMenu {
    type OnInputReturn = OnInputFlag;
    fn on_input(&mut self, evt: InputEvt) -> OnInputFlag {
        let old_selected_side_menu = self.selected;
        match evt {
            InputEvt::CounterClockwise => {
                if self.selected > 0.into() {
                    self.selected -= 1;
                    self.changed = true;
                }
            }
            InputEvt::Clockwise => {
                if self.selected < (Selection::LEN as u8-1).into() {
                    self.selected += 1;
                    self.changed = true;
                }
            }
            InputEvt::Enter => {
                return match self.selected {
                    Selection::Sensor => OnInputFlag::ToSensorMenu,
                    Selection::Fan => OnInputFlag::ToFanMenu,
                    Selection::Wifi => OnInputFlag::None,
                }
            }
        }
        if self.changed {
            match self.side_menu_lerp.take() {
                Some(old_lerp) => {
                    let _ = self.side_menu_lerp.insert(Lerp::new(
                        old_lerp.get(),
                        self.selected as u8 as f32,
                        SIDE_MENU_LERP_DURATION,
                    ));
                }
                None => {
                    let _ = self.side_menu_lerp.insert(Lerp::new(
                        old_selected_side_menu as u8 as f32,
                        self.selected as u8 as f32,
                        SIDE_MENU_LERP_DURATION,
                    ));
                }
            }
        }
        OnInputFlag::None
    }
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
