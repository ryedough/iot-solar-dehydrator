use core::ops::{Add, AddAssign, Sub, SubAssign};

use embedded_graphics::{Drawable, mono_font::{self, MonoTextStyle}, pixelcolor::BinaryColor, prelude::*, primitives::{Rectangle, StyledDrawable}, text::Text};

use crate::{DISPLAY_WIDTH, InputEvt, PRIMITIVE_STYLE_BORDER_ONLY, PRIMITIVE_STYLE_ON, animation::FlushableDisplay, menu::BareMenu};

#[derive(Clone, Copy, PartialEq)]
pub enum FanSpeed {
    Low = 0,
    Medium,
    High,
    Max,
}

impl FanSpeed {
    const LEN : usize = 4;
}

impl Add<u8> for FanSpeed {
    type Output = Self;
    fn add(self, rhs: u8) -> Self::Output {
        let mut v = self as u8 + rhs;
        if v > Self::LEN as u8 -1 {
            v = Self::LEN as u8-1
        };
        v.try_into().unwrap()
    }
}
impl Into<&str> for FanSpeed {
    fn into(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Max => "Max",
        }
    }
}

impl Sub<u8> for FanSpeed {
    type Output = Self;
    fn sub(self, rhs: u8) -> Self::Output {
        let v = if self as u8 > 0 {self as u8 - rhs} else {0};
        v.try_into().unwrap()
    }
}

impl SubAssign<u8> for FanSpeed {
    fn sub_assign(&mut self, rhs: u8) {
        *self = (*self - rhs).try_into().unwrap()
    }
}
impl AddAssign<u8> for FanSpeed {
    fn add_assign(&mut self, rhs: u8) {
        *self = (*self + rhs).try_into().unwrap()
    }
}

impl TryFrom<u8> for FanSpeed {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Low),
            1 => Ok(Self::Medium),
            2 => Ok(Self::High),
            3 => Ok(Self::Max),
            _ => Err(())
        }
    }
}

pub enum OnInputFlag {
    BackToMenu,
    Save(FanSpeed),
    None,
}

pub struct FanMenu {
    saved_fan_speed : FanSpeed,
    selected : FanSpeed,
    changed : bool,
    on_char : MonoTextStyle<'static, BinaryColor>,
    off_char : MonoTextStyle<'static, BinaryColor>
}

impl FanMenu {
    pub fn new(saved_fan_speed : FanSpeed)->Self {
        Self{
            selected : 0.try_into().unwrap(),
            saved_fan_speed,
            changed : true,
            on_char : MonoTextStyle::new(&mono_font::ascii::FONT_6X12, BinaryColor::On),
            off_char : MonoTextStyle::new(&mono_font::ascii::FONT_6X12, BinaryColor::Off),
        }
    }
    async fn render<T: FlushableDisplay>(&mut self, display : &mut T){
        display.clear(BinaryColor::Off).unwrap();
        let display_center = DISPLAY_WIDTH as i32 /2;

        Text::with_alignment(
            "Select Fan Speed",
            Point::new(display_center, 6),
            self.on_char,
            embedded_graphics::text::Alignment::Center
        ).draw(display).unwrap();
        Rectangle::new(Point::new(3, 10), Size::new(DISPLAY_WIDTH as u32- 6, 2))
            .draw_styled(&PRIMITIVE_STYLE_ON, display).unwrap();

        let selection_y = 12;
        let selection_gap_y = 13;
        for i in 0..FanSpeed::LEN {
            if self.saved_fan_speed == (i as u8).try_into().unwrap() {
                Rectangle::new(Point::new(3, selection_y + selection_gap_y * i as i32 + 1), Size::new(DISPLAY_WIDTH as u32- 6, selection_gap_y as u32 -1))
                    .draw_styled(&PRIMITIVE_STYLE_ON, display).unwrap();
            } else if self.selected == (i as u8).try_into().unwrap() {
                Rectangle::new(Point::new(3, selection_y + selection_gap_y * i as i32 + 1 ), Size::new(DISPLAY_WIDTH as u32- 6, selection_gap_y as u32 -1))
                    .draw_styled(&PRIMITIVE_STYLE_BORDER_ONLY, display).unwrap();
            }
            Text::with_alignment(
                FanSpeed::try_from(i as u8).unwrap().try_into().unwrap(),
                Point::new(display_center, 9 + selection_y + (selection_gap_y * i as i32)),
                if self.saved_fan_speed == (i as u8).try_into().unwrap() {
                    self.off_char
                } else {
                    self.on_char
                },
                embedded_graphics::text::Alignment::Center
            ).draw(display).unwrap();
        }
        display.flush().await.unwrap()
    }
}

impl BareMenu for FanMenu {
    type OnInputReturn = OnInputFlag;
    async fn tick(&mut self, display: &mut impl FlushableDisplay) {
        if self.changed {
            self.changed = false;
            self.render(display).await;
        }
    }
    fn on_input(&mut self, evt: crate::InputEvt) -> Self::OnInputReturn {
        self.changed = true;
        match &evt {
            InputEvt::CounterClockwise => {
                if self.selected as usize == 0 {
                    self.selected = (FanSpeed::LEN as u8-1).try_into().unwrap();
                } else {
                    self.selected -= 1;
                }
            },
            InputEvt::Clockwise => {
                if self.selected as u8 >= FanSpeed::LEN as u8-1 {
                    self.selected = 0.try_into().unwrap();
                } else {
                    self.selected += 1;
                }
            },
            InputEvt::Enter => if self.selected != self.saved_fan_speed {
                return OnInputFlag::Save(self.selected);
            } else {
                return OnInputFlag::BackToMenu;
            }
        };

        OnInputFlag::None
    }
}
