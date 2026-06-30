use core::{fmt::Write, ops::{Add, AddAssign, Sub, SubAssign}};

use embedded_graphics::{mono_font::MonoTextStyle, pixelcolor::BinaryColor, prelude::*, primitives::{PrimitiveStyle, Rectangle, StyledDrawable}, text::Text};
use heapless::String;

use crate::{InputEvt, animation::FlushableDisplay, menu::BareMenu, sht31::SHT31Reading};

#[derive(Clone, Copy, PartialEq)]
enum Selection {
    Temperature = 0,
    Humidity = 1,
}
impl Selection {
    const LEN : u8 = 2;
}
impl From<u8> for Selection {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Temperature,
            1 => Self::Humidity,
            _ => panic!("invalid SensorMenu Selection"),
        }
    }
}

impl Add<u8> for Selection {
    type Output = Self;
    fn add(self, rhs: u8) -> Self::Output {
        let mut v = self as u8 + rhs;
        if v > Self::LEN -1 {
            v = Self::LEN-1
        };
        v.into()
    }
}

impl Sub<u8> for Selection {
    type Output = Self;
    fn sub(self, rhs: u8) -> Self::Output {
        let mut v = self as u8 - rhs;
        if v < 0 {
            v = 0;
        };
        v.into()
    }
}

impl SubAssign<u8> for Selection {
    fn sub_assign(&mut self, rhs: u8) {
        *self = (*self - rhs).into()
    }
}
impl AddAssign<u8> for Selection {
    fn add_assign(&mut self, rhs: u8) {
        *self = (*self + rhs).into()
    }
}

enum Mode {
    Selection,
    Editing,
}

pub enum OnInputFlag {
    Save(SHT31Reading),
    None
}

pub struct SensorMenu{
    selection : Selection,
    text_char : MonoTextStyle<'static, BinaryColor>,
    calibration : SHT31Reading,
    changed : bool,
    mode : Mode,
}
impl SensorMenu{
    pub fn new(calibration : SHT31Reading)->Self{
        Self{
            selection: 0.try_into().unwrap(),
            calibration,
            text_char: MonoTextStyle::new(
                &embedded_graphics::mono_font::ascii::FONT_6X12,
                BinaryColor::On,
            ),
            changed : true,
            mode : Mode::Selection,
        }
    }
    async fn render(&mut self, display : &mut impl FlushableDisplay){
        display.clear(BinaryColor::Off).unwrap();
        let style = PrimitiveStyle::with_fill(BinaryColor::On);

        let selection_pad = 3;
        let selection_w = 24;

        let temp_y = 6;
        let temp_value_x = 70;
        Text::new("Temperature", Point::new(0, temp_y), self.text_char)
            .draw(display)
            .unwrap();

        let mut temp : String<4> = String::new();
        let plus_sign = if self.calibration.temp > 0. {
            "+"
        } else {""};
        write!(temp, "{}{:.1}", plus_sign,self.calibration.temp).unwrap();
        Text::new(&temp, Point::new(temp_value_x, temp_y), self.text_char)
            .draw(display)
            .unwrap();
        if self.selection == Selection::Temperature {
            Rectangle::new(Point::new(temp_value_x - 1, temp_y + selection_pad), Size::new(selection_w, 1))
                .draw_styled(&style, display).unwrap();
        }

        let humid_y = 19;
        let humid_val_x = 70;
        Text::new("Humidity", Point::new(0, humid_y), self.text_char)
            .draw(display)
            .unwrap();
        let mut humid : String<4> = String::new();
        let plus_sign = if self.calibration.humid > 0. {
            "+"
        } else {""};
        write!(humid, "{}{:.1}", plus_sign,self.calibration.humid).unwrap();
        Text::new(&humid, Point::new(humid_val_x, humid_y), self.text_char)
            .draw(display)
            .unwrap();
        if self.selection == Selection::Humidity {
            Rectangle::new(Point::new(humid_val_x - 1, humid_y + selection_pad), Size::new(selection_w, 1))
                .draw_styled(&style, display).unwrap();
        }

        display.flush().await.unwrap();
    }

    pub fn on_input(&mut self, evt: crate::InputEvt) -> OnInputFlag {
        self.changed = true;
        match &self.mode {
            Mode::Selection => {
                match &evt {
                    InputEvt::Up => {
                        if self.selection as usize == 0 {
                            self.selection = (Selection::LEN-1).into();
                        } else {
                            self.selection -= 1;
                        }
                    },
                    InputEvt::Down => {
                        if self.selection as u8 >= Selection::LEN-1 {
                            self.selection = 0.into();
                        } else {
                            self.selection += 1;
                        }
                    },
                    InputEvt::Enter => self.mode = Mode::Editing,
                }
            },
            Mode::Editing => {
                match &evt {
                    InputEvt::Up => {
                        match self.selection {
                            Selection::Temperature => {
                                self.calibration.temp = if self.calibration.temp < 9.9 {
                                    self.calibration.temp + 0.1
                                } else {
                                    9.9
                                }
                            },
                            Selection::Humidity => {
                                self.calibration.humid = if self.calibration.humid < 9.9 {
                                    self.calibration.humid + 0.1
                                } else {
                                    9.9
                                }
                            }
                        }
                    },
                    InputEvt::Down => {
                        match self.selection {
                            Selection::Temperature => {
                                self.calibration.temp = if self.calibration.temp > -9.9 {
                                    self.calibration.temp - 0.1
                                } else {
                                    -9.9
                                }
                            },
                            Selection::Humidity => {
                                self.calibration.humid = if self.calibration.humid > -9.9 {
                                    self.calibration.humid - 0.1
                                } else {
                                    -9.9
                                }
                            }
                        }
                    },
                    InputEvt::Enter => self.mode = Mode::Selection,
                }
            }
        }
        OnInputFlag::None
    }
}
impl BareMenu for SensorMenu {
    async fn tick(&mut self, display: &mut impl FlushableDisplay) {
        if self.changed {
            self.changed = false;
            self.render(display).await;
        }
    }
}
