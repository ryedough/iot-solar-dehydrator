use defmt::info;
use embassy_stm32::i2c::{Error, I2c};
use embedded_graphics::{draw_target::DrawTarget, pixelcolor::BinaryColor};
use embedded_graphics::prelude::*;

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

    async fn write_raw<const LEN : usize, const DATA_LEN : usize>(iface : &mut SSD1315Iface, self_addr : u8, cmds : &[u8; LEN], t : WriteType) -> Result<(), Error> {
        assert!(DATA_LEN == LEN * 2);
        let control_byte : u8 = match t{
            WriteType::Command => 0x2,
            WriteType::Data => 0x3
        } << 6;
        let cmds = cmds.iter().copied().flat_map(|x| [control_byte, x]);
        let cmds : heapless::Vec<u8, DATA_LEN> = heapless::Vec::from_iter(cmds);
        iface.write(self_addr, &cmds).await
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
        Self::write_raw::<3, {3*2}>(&mut self.iface, self.addr, &cmd, WriteType::Command).await
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

    pub async fn flush(&mut self) -> Result<(), Error> {
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
        Self::write_raw::<{128 * 8}, {128 * 8 * 2}>(&mut self.iface, self.addr, &framebuffer,WriteType::Data).await
    }

    pub async fn init(&mut self) -> Result<(), Error> {
        let cmds : [u8; _] = [
            0xA8, 0x3F, // Set Mux Ratio
            0xD3, 0x00, // Set Display offset
            0x20, 0x00, // Set Adressing mode to vertical
            0x40,       // Set start line
            0xA0,       // Set segment re-map / 0xA1
            0xC0,       // Set COM output scan direction / 0xC8
            0xDA, 0x12, // Set COM pin hardware configuration
            0x81, 0x7F, // Set contrast
            0xA4,       // Resume the display
            0xD5, 0x80, // Set Oscillator frequency
            0x8D, 0x14, // Enable Charge pump
            0xAF        // Turn the display on
        ];
        Self::write_raw::<19, {19 * 2}>(&mut self.iface, self.addr, &cmds, WriteType::Command).await
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
            if let Ok((x @ 0..=63, y @ 0..=63)) = coord.try_into() {
                self.set_pixel(x as u8, y as u8, color.is_on());
            }
        };
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
