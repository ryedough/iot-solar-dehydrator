use defmt::info;
use embassy_stm32::i2c::{Error, I2c};
use embedded_graphics::{draw_target::DrawTarget, pixelcolor::BinaryColor};
use embedded_graphics::prelude::*;
use embedded_hal_async::i2c::Operation;
use crate::animation::FlushableDisplay;

type SSD1315Iface = I2c<'static, embassy_stm32::mode::Async, embassy_stm32::i2c::Master>;

pub struct SSD1315 {
    framebuffer: [u8; 128 * 8],
    iface: SSD1315Iface,
    addr : u8,
}

enum SetAddress {
    Page{start : u8, end : u8},
    Column{start : u8, end: u8},
}

enum WriteType {
    Command,
    Data,
}

impl SSD1315 {
    pub fn new(iface : SSD1315Iface)->Self {
        Self {
            framebuffer : [0; 128 * 8],
            addr : 0x3C,
            iface
        }
    }

    async fn write_raw(iface : &mut SSD1315Iface, self_addr : u8, cmds : &[u8], t : WriteType) -> Result<(), Error> {
        let ctrl: u8 = match t{
            WriteType::Command => 0x00,
            WriteType::Data => 0x40
        };
        iface.transaction(self_addr, &mut [
            Operation::Write(&[ctrl]),
            Operation::Write(cmds),
        ]).await
    }

    async fn set_addr(&mut self, address : SetAddress) -> Result<(), Error>{
        let cmd = match address {
            SetAddress::Page { start, end } => [
                0x22,
                start & 0x07,
                end & 0x07,
            ],
            SetAddress::Column { start, end } => [
                0x21,
                start & 0x7F,
                end & 0x7F,
            ]
        };
        Self::write_raw(&mut self.iface, self.addr, &cmd, WriteType::Command).await
    }

    pub fn set_pixel(&mut self, x :u8, y :u8, value : bool) {
        assert!(x < 128 && y < 64);
        let target : usize = (usize::from(y)/8) * 128 + usize::from(x);
        let target = unsafe {self.framebuffer.get_unchecked_mut(target)};
        if value {
            *target |= 0x1 << (y % 8);
        } else {
            *target &= !(0x1 << (y % 8));
        };
    }


    pub async fn init(&mut self) -> Result<(), Error> {
        let cmds : [u8; _] = [
            0xA8, 0x3F, // Set Mux Ratio
            0xD3, 0x00, // Set Display offset
            0x20, 0x00, // Set Adressing mode to vertical
            0x40,       // Set start line
            0xA1,       // Set segment re-map / 0xA0
            0xC8,       // Set COM output scan direction / 0xC0
            0xDA, 0x12, // Set COM pin hardware configuration
            0x81, 0x7F, // Set contrast
            0xA4,       // Resume the display
            0xD5, 0x80, // Set Oscillator frequency
            0x8D, 0x14, // Enable Charge pump
            0xAF        // Turn the display on
        ];
        Self::write_raw(&mut self.iface, self.addr, &cmds, WriteType::Command).await
    }
}

impl DrawTarget for SSD1315 {
    type Color = BinaryColor;
    type Error = Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>
    {
        for Pixel(coord, color) in pixels.into_iter() {
            if let Ok((x @ 0..128, y @ 0..64)) = coord.try_into() {
                self.set_pixel(x as u8, y as u8, color.is_on());
            }
        };
        Ok(())
    }
    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.framebuffer.fill(match color.is_on() {
            true => 0xff,
            false => 0
        });
        Ok(())
    }
}

impl OriginDimensions for SSD1315 {
    fn size(&self) -> Size {
        Size {
            width : 128,
            height: 64
        }
    }
}

impl FlushableDisplay for SSD1315 {
    async fn flush(&mut self) -> Result<(), Error> {
        self.set_addr(SetAddress::Column { start: 0, end: 127 }).await.expect("Flush: set column shouldn't err");
        self.set_addr(SetAddress::Page { start: 0, end: 7 }).await.expect("Flush: set page shouldn't err");

        let framebuffer = &self.framebuffer;
        // let mut f_debug: heapless::Vec<u8, 8> = heapless::Vec::new();
        // for f in framebuffer {
        //     f_debug.push(f.clone()).expect("should always success");
        //     if f_debug.is_full()  {
        //         info!("{:?},", f_debug);
        //         f_debug.clear();
        //     }
        // }
        Self::write_raw(&mut self.iface, self.addr, framebuffer,WriteType::Data).await
    }
}
