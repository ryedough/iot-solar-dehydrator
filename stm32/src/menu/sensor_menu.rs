use core::{fmt::Write, ops::{Add, AddAssign, Sub, SubAssign}};

use embassy_time::{Duration, Instant};
use embedded_graphics::{mono_font::MonoTextStyle, pixelcolor::BinaryColor, prelude::*, primitives::{PrimitiveStyle, Rectangle, StyledDrawable}, text::Text};
use heapless::String;

use crate::{DISPLAY_HEIGHT, DISPLAY_WIDTH, InputEvt, PRIMITIVE_STYLE_BORDER_ONLY, PRIMITIVE_STYLE_ON, animation::FlushableDisplay, menu::BareMenu, sht31::SHT31Reading};

#[derive(Clone, Copy, PartialEq)]
enum Selection {
    Temperature = 0,
    Humidity = 1,
    Done=2,
    Cancel=3,
}
impl Selection {
    const LEN : u8 = 4;
}
impl From<u8> for Selection {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Temperature,
            1 => Self::Humidity,
            2 => Self::Done,
            3 => Self::Cancel,
            _ => panic!("invalid SensorMenu Selection, probably Selection changed but not it's From<u8>"),
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
        let v = if self as u8 > 0 {self as u8 - rhs} else {0};
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

enum ChangeHold {
    Add(f32),
    Sub(f32),
}

struct ChangeHoldAt {
    at : Instant,
    change_hold : ChangeHold,
}

#[derive(PartialEq)]
enum Mode {
    Selection,
    Editing,
}

pub enum OnInputFlag {
    Save(SHT31Reading),
    BackToMain,
    None
}


pub struct SensorMenu{
    selection : Selection,
    text_char : MonoTextStyle<'static, BinaryColor>,
    info_char : MonoTextStyle<'static, BinaryColor>,
    calibration : SHT31Reading,
    changed : bool,
    change_hold : Option<ChangeHoldAt>,
    mode : Mode,
}
impl SensorMenu{
    const CHANGE_HOLD_EXPIRE : Duration = Duration::from_millis(200);
    pub fn new(calibration : SHT31Reading)->Self{
        Self{
            selection: 0.try_into().unwrap(),
            calibration,
            text_char: MonoTextStyle::new(
                &embedded_graphics::mono_font::ascii::FONT_6X12,
                BinaryColor::On,
            ),
            change_hold : None,
            info_char: MonoTextStyle::new(
                &embedded_graphics::mono_font::ascii::FONT_6X12,
                BinaryColor::Off
            ),
            changed : true,
            mode : Mode::Selection,
        }
    }
    async fn render(&mut self, display : &mut impl FlushableDisplay){
        display.clear(BinaryColor::Off).unwrap();

        let selection_pad = 3;
        let selection_w = 24;

        // draw temperature setting
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
                .draw_styled(&PRIMITIVE_STYLE_ON, display).unwrap();
        }

        // draw humidity setting
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
                .draw_styled(&PRIMITIVE_STYLE_ON, display).unwrap();
        }

        // draw done & cancel button -------------------------------------------------
        let btn_y = humid_y + 7;

        // draw done button
        let done_btn_style = match self.selection {
            Selection::Done => &PRIMITIVE_STYLE_ON,
            _ => &PRIMITIVE_STYLE_BORDER_ONLY,
        };
        let done_txt_style = match self.selection {
            Selection::Done => self.info_char,
            _ => self.text_char,
        };
        Rectangle::new(Point::new(0,btn_y), Size::new(30, 11))
            .draw_styled(done_btn_style, display).unwrap();
        Text::new("Done", Point::new(3,btn_y + 8), done_txt_style)
            .draw(display).unwrap();

        //draw cancel button
        let cancel_btn_style = match self.selection {
            Selection::Cancel => &PRIMITIVE_STYLE_ON,
            _ => &PRIMITIVE_STYLE_BORDER_ONLY,
        };
        let cancel_txt_style = match self.selection {
            Selection::Cancel => self.info_char,
            _ => self.text_char,
        };
        let cancel_x = 34;
        Rectangle::new(Point::new(cancel_x,btn_y), Size::new(40, 11))
            .draw_styled(cancel_btn_style, display).unwrap();
        Text::new("Cancel", Point::new(cancel_x + 3,btn_y + 8), cancel_txt_style)
            .draw(display).unwrap();

        // draw info ------------------------------------------------------------
        match self.selection {
            Selection::Temperature | Selection::Humidity => {
                let info_y = DISPLAY_HEIGHT as i32- 10;
                Rectangle::with_corners(
                    Point::new(0, info_y),
                    Point::new(DISPLAY_WIDTH as i32, DISPLAY_HEIGHT as i32)
                    ).draw_styled(&PRIMITIVE_STYLE_ON, display).unwrap();

                if self.mode == Mode::Selection {
                    Text::new("Press button to edit", Point::new(2, info_y + 7 ), self.info_char)
                        .draw(display)
                        .unwrap();
                } else {
                    Text::new("Editing", Point::new(2, info_y + 7 ), self.info_char)
                        .draw(display)
                        .unwrap();
                }
            },
            _ => ()
        }

        display.flush().await.unwrap();
    }
}
impl BareMenu for SensorMenu {
    type OnInputReturn = OnInputFlag;
    async fn tick(&mut self, display: &mut impl FlushableDisplay) {
        if self.changed {
            self.changed = false;
            self.render(display).await;
        }
    }
    fn on_input(&mut self, evt: crate::InputEvt) -> OnInputFlag {
        self.changed = true;
        match &self.mode {
            Mode::Selection => {
                self.change_hold = None;
                match &evt {
                    InputEvt::CounterClockwise => {
                        if self.selection as usize == 0 {
                            self.selection = (Selection::LEN-1).into();
                        } else {
                            self.selection -= 1;
                        }
                    },
                    InputEvt::Clockwise => {
                        if self.selection as u8 >= Selection::LEN-1 {
                            self.selection = 0.into();
                        } else {
                            self.selection += 1;
                        }
                    },
                    InputEvt::Enter => match self.selection {
                        Selection::Temperature | Selection::Humidity => self.mode = Mode::Editing,
                        Selection::Done => return OnInputFlag::Save(self.calibration.clone()),
                        Selection::Cancel => return OnInputFlag::BackToMain,
                    },
                }
            },
            Mode::Editing => {
                match self.change_hold.as_ref() {
                    Some(change_hold) => {
                        if change_hold.at.elapsed() > Self::CHANGE_HOLD_EXPIRE {
                            self.change_hold = None;
                        }
                    },
                    None => {},
                }
                match &evt {
                    InputEvt::Clockwise => {
                        // check if user do rapid change
                        let accel = if let Some(change_hold_at) = self.change_hold.as_mut() {
                            change_hold_at.at = Instant::now();
                            if let ChangeHold::Add(accel) = &mut change_hold_at.change_hold {
                                let acc = *accel;
                                *accel += 0.1;
                                acc
                            } else {
                                change_hold_at.change_hold = ChangeHold::Add(0.);
                                0.
                            }
                        } else {
                            self.change_hold = Some(ChangeHoldAt{
                                change_hold : ChangeHold::Add(0.),
                                at : Instant::now()
                            });
                            0.
                        };
                        match self.selection {
                            Selection::Temperature => {
                                self.calibration.temp = (self.calibration.temp + 0.1 + accel).clamp(-9.9, 9.9);
                            },
                            Selection::Humidity => {
                                self.calibration.humid = (self.calibration.humid + 0.1 + accel).clamp(-9.9, 9.9)
                            },
                            _ => panic!("illegal editing selection value")
                        }
                    },
                    InputEvt::CounterClockwise => {
                        // check if user do rapid change
                        let accel = if let Some(change_hold_at) = self.change_hold.as_mut() {
                            change_hold_at.at = Instant::now();
                            if let ChangeHold::Sub(accel) = &mut change_hold_at.change_hold {
                                let acc = *accel;
                                *accel += 0.1;
                                acc
                            } else {
                                change_hold_at.change_hold = ChangeHold::Sub(0.);
                                0.
                            }
                        } else {
                            self.change_hold = Some(ChangeHoldAt{
                                change_hold : ChangeHold::Sub(0.),
                                at : Instant::now()
                            });
                            0.
                        };
                        match self.selection {
                            Selection::Temperature => {
                                self.calibration.temp = (self.calibration.temp - 0.1 - accel).clamp(-9.9, 9.9);
                            },
                            Selection::Humidity => {
                                self.calibration.humid = (self.calibration.humid - 0.1 - accel).clamp(-9.9, 9.9)
                            },
                            _ => panic!("illegal editing selection value")
                        }
                    },
                    InputEvt::Enter => self.mode = Mode::Selection,
                }
            }
        }
        OnInputFlag::None
    }
}
