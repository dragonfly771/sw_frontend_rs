// Display variants (exactly one is built, see make_variant.sh):
//   small  (default)                  round series display of the LARUS vario display
//   large  (feature "display_large")  2.76" IPS 480x480, Zhunyi Z28016-P40-771S-Y1, also ST7701S
// Both use the same pins, the same RGB666 bus and the same resolution, only the init sequence
// of the driver IC and the panel timing (ltdc.rs) differ.
//
// This initialization sequence comes from the manufacturer of the driver IC. It works, but is not
// consistently comprehensible in relation to the data sheet.
//
// This data is for initializing the st7701 driver:
// 0x00xx   cmd xx
// 0x01xx   data xx
#[cfg(not(feature = "display_large"))]
#[rustfmt::skip]
const INIT_SEQ_1: [u16; 206] = [
    0x00FF, // cmd Command2 BKx Selection CN2 = 1 BKx = 0
    0x0177, 0x0101, 0x0100, 0x0100, 0x0110,

    0x00C0, // cmd Display Line Setting 
    0x013B, 0x0100, 
    
    0x00C1, // cmd Porch Control, VBP
    0x010B, 0x0102, 
    
    0x00C2, // cmd Inversion selection & Frame Rate Control
    0x0100, 0x0102, 
    
    0x00CC,  // cmd ??? CC is a BK3 Command NVM Program Active
    0x0110, 
    
    0x00CD, // cmd Color Control
    0x0108, 
    
    0x00B0, // cmd Positive Voltage Gamma Control
    0x0102, 0x0113, 0x011B, 0x010D, 0x0110, 0x0105, 0x0108, 0x0107, 
    0x0107, 0x0124, 0x0104, 0x0111, 0x010E, 0x012C, 0x0133, 0x011D, 
    
    0x00B1, // cmd Negative Voltage Gamma Control
    0x0105, 0x0113, 0x011B, 0x010D, 0x0111, 0x0105, 0x0108, 0x0107, 
    0x0107, 0x0124, 0x0104, 0x0111, 0x010E, 0x012C, 0x0133, 0x011D, 
    
    0x00FF, // cmd Command2 BKx Selection
    0x0177, 0x0101, 0x0100, 0x0100, 0x0111, // CN2 = 1 BKx = 1
    
    0x00B0, // cmd VOP amplitude setting
    0x015d, // 5d
    
    0x00B1, // cmd VCOM amplitude setting
    0x0143, // 43
    
    0x00B2, // cmd VGH Voltage setting
    0x0181, // 12V
    
    0x00B3, // cmd Test Command Setting
    0x0180, 
    
    0x00B5, // cmd VGL Voltage setting
    0x0143, // -8.3V
    
    0x00B7, // cmd Power Control 1
    0x0185, 
    
    0x00B8, // cmd Power Control 2
    0x0120, 
    
    0x00C1, // cmd Source pre_drive timing set1
    0x0178, 
    
    0x00C2, // cmd Source pre_drive timing set2
    0x0178, 
    
    0x00D0, // cmd MIPI Setting 1
    0x0188,
    
    0x00E0, // cmd ??? BK0 Sunlight Readable Enhancement
    0x0100, 0x0100, 0x0102, 
    
    0x00E1, // cmd ??? BK0 Noise Reduce Control
    0x0103, 0x01A0, 0x0100, 0x0100, 0x0104, 0x01A0, 0x0100, 0x0100, 0x0100, 0x0120, 0x0120,
    
    0x00E2, // cmd ??? BK0 Sharpnes Control
    0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 
    0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 
    
    0x00E3, // cmd ??? BK0 Color Calibration Control
    0x0100, 0x0100, 0x0111, 0x0100, 
    
    0x00E4, // cmd ??? BKß Skin Tone Preservation Control
    0x0122, 0x0100, 
    
    0x00E5, // cmd ??? E5 undefined
    0x0105, 0x01EC, 0x01A0, 0x01A0, 0x0107, 0x01EE, 0x01A0, 0x01A0, 
    0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 
    
    0x00E6, // cmd ??? E6 undefined
    0x0100, 0x0100, 0x0111, 0x0100, 
    
    0x00E7, // cmd ??? E7 undefined
    0x0122, 0x0100, 
    
    0x00E8, // cmd ??? E8 undefined
    0x0106, 0x01ED, 0x01A0, 0x01A0, 0x0108, 0x01EF, 0x01A0, 0x01A0, 
    0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 
    
    0x00EB, // cmd ??? EB undefined
    0x0100, 0x0100, 0x0140, 0x0140, 0x0100, 0x0100, 0x0100, 
    
    0x00ED, // cmd ??? undefined
    0x01FF, 0x01FF, 0x01FF, 0x01BA, 0x010A, 0x01BF, 0x0145, 0x01FF, 
    0x01FF, 0x0154, 0x01FB, 0x01A0, 0x01AB, 0x01FF, 0x01FF, 0x01FF, 

    0x00EF, // cmd ??? undefined
    0x0110, 0x010D, 0x0104, 0x0108, 0x013F, 0x011F, 
    
    0x00FF, // cmd BK3 Selected
    0x0177, 0x0101, 0x0100, 0x0100, 0x0113, 
    
    0x00EF, // cmd ??? undefined
    0x0108, 
    
    0x00FF, // cmd BK0 Selected
    0x0177, 0x0101, 0x0100, 0x0100, 0x0100, 
    
    0x0011, // cmd ??? undefined
];

#[cfg(not(feature = "display_large"))]
#[rustfmt::skip]
const INIT_SEQ_2: [u16; 5] = [
    0x0029, // cmd ??? undefined

    0x0036, // cmd ??? undefined
    0x0100, 
    
    0x003A, // cmd ??? undefined
    0x0160,
];

// Large display (2.76", Z28016): init code of the panel manufacturer, unchanged
// ("2.76IPS+ST7701 initial code", 18.09.). It sets the scan direction to C7h = 0x04 and
// 36h = 0x10, i.e. both axes turned compared to the small display (= 180 degrees). If the
// picture is upside down, the rotation in the menu (Display > Rotation) fixes it.
// Unlike the small display it does not write COLCTRL (CDh = 0x08, MDT: pixel data on
// DB[17:0]). If the colours are wrong (mostly dark, odd colours), insert
// `0x00CD, 0x0108,` after the C7h entry - that is the only known difference on the bus side.
#[cfg(feature = "display_large")]
#[rustfmt::skip]
const INIT_SEQ_1: [u16; 208] = [
    0x00FF, // Command2 BKx selection -> 0x13
    0x0177, 0x0101, 0x0100, 0x0100, 0x0113,

    0x00EF, // (BK3/BK1) vendor setting
    0x0108,

    0x00FF, // Command2 BKx selection -> 0x10
    0x0177, 0x0101, 0x0100, 0x0100, 0x0110,

    0x00C0, // Display line setting: (0x3B+1)*8 = 480 lines
    0x013B, 0x0100,

    0x00C1, // Porch control VBP/VFP
    0x0110, 0x010C,

    0x00C2, // Inversion (column) & min. 512+10*16 = 672 pclk per line
    0x0107, 0x010A,

    0x00C7, // SDIR: source direction, 0x04 = 479..0 (left/right)
    0x0104,

    0x00CC, // vendor setting
    0x0110,

    0x00B0, // Positive voltage gamma / (BK1) VOP amplitude
    0x0105, 0x0112, 0x0198, 0x010E, 0x010F, 0x0107, 0x0107, 0x0109,
    0x0109, 0x0123, 0x0105, 0x0152, 0x010F, 0x0167, 0x012C, 0x0111,

    0x00B1, // Negative voltage gamma / (BK1) VCOM amplitude
    0x010B, 0x0111, 0x0197, 0x010C, 0x0112, 0x0106, 0x0106, 0x0108,
    0x0108, 0x0122, 0x0103, 0x0151, 0x0111, 0x0166, 0x012B, 0x010F,

    0x00FF, // Command2 BKx selection -> 0x11
    0x0177, 0x0101, 0x0100, 0x0100, 0x0111,

    0x00B0, // Positive voltage gamma / (BK1) VOP amplitude
    0x015D,

    0x00B1, // Negative voltage gamma / (BK1) VCOM amplitude
    0x0135,

    0x00B2, // VGH voltage
    0x0181,

    0x00B3, // Test command setting
    0x0180,

    0x00B5, // VGL voltage
    0x014E,

    0x00B7, // Power control 1
    0x0185,

    0x00B8, // Power control 2
    0x0120,

    0x00C1, // Porch control VBP/VFP
    0x0178,

    0x00C2, // Inversion (column) & min. 512+10*16 = 672 pclk per line
    0x0178,

    0x00D0, // MIPI setting 1
    0x0188,

    0x00E0, // GIP / vendor
    0x0100, 0x0100, 0x0102,

    0x00E1, // GIP / vendor
    0x0106, 0x0130, 0x0108, 0x0130, 0x0105, 0x0130, 0x0107, 0x0130,
    0x0100, 0x0133, 0x0133,

    0x00E2, // GIP / vendor
    0x0111, 0x0111, 0x0133, 0x0133, 0x01F4, 0x0100, 0x0100, 0x0100,
    0x01F4, 0x0100, 0x0100, 0x0100,

    0x00E3, // GIP / vendor
    0x0100, 0x0100, 0x0111, 0x0111,

    0x00E4, // GIP / vendor
    0x0144, 0x0144,

    0x00E5, // GIP / vendor
    0x010D, 0x01F5, 0x0130, 0x01F0, 0x010F, 0x01F7, 0x0130, 0x01F0,
    0x0109, 0x01F1, 0x0130, 0x01F0, 0x010B, 0x01F3, 0x0130, 0x01F0,

    0x00E6, // GIP / vendor
    0x0100, 0x0100, 0x0111, 0x0111,

    0x00E7, // GIP / vendor
    0x0144, 0x0144,

    0x00E8, // GIP / vendor
    0x010C, 0x01F4, 0x0130, 0x01F0, 0x010E, 0x01F6, 0x0130, 0x01F0,
    0x0108, 0x01F0, 0x0130, 0x01F0, 0x010A, 0x01F2, 0x0130, 0x01F0,

    0x00E9, // GIP / vendor
    0x0136, 0x0101,

    0x00EB, // GIP / vendor
    0x0100, 0x0101, 0x01E4, 0x01E4, 0x0144, 0x0188, 0x0140,

    0x00ED, // GIP / vendor
    0x01FF, 0x0110, 0x01BF, 0x0176, 0x0154, 0x012A, 0x01FC, 0x01FF,
    0x01FF, 0x01CF, 0x01A2, 0x0145, 0x0167, 0x01FB, 0x0101, 0x01FF,

    0x00EF, // (BK3/BK1) vendor setting
    0x0108, 0x0108, 0x0108, 0x0145, 0x013F, 0x0154,

    0x00FF, // Command2 BKx selection -> 0x00
    0x0177, 0x0101, 0x0100, 0x0100, 0x0100,

    0x0011, // Sleep out (120 ms delay follows)
];

#[cfg(feature = "display_large")]
#[rustfmt::skip]
const INIT_SEQ_2: [u16; 7] = [
    0x003A, // COLMOD: 0x66 = RGB666 (18 bit)
    0x0166,

    0x0036, // MADCTL: 0x10 = line order reversed (top/bottom)
    0x0110,

    0x0035, // Tearing effect on (pin not used)
    0x0100,

    0x0029, // Display on
];

use super::Delay;
use stm32h7xx_hal::{
    gpio::{self, Output, PinState},
    pac,
    prelude::*,
    rcc::{rec, CoreClocks, ResetEnable},
};

// These pins are required to control the SPI interface of the driver IC and to reset it.
pub struct LcdPins(
    pub gpio::Pin<'B', 5>,  // SPI MOSI
    pub gpio::Pin<'G', 9>,  // SPI MISO
    pub gpio::Pin<'G', 11>, // SPI SLK
    pub gpio::Pin<'H', 4>,  // SPI_CS
    pub gpio::Pin<'H', 3>,  // LCD Reset
);

pub struct St7701s {}

// This driver only initializes the driver IC ST7701S. The image data is then transferred from the
// LTDC periphery to the LCD via RGB, VSync, HSync etc.
impl St7701s {
    pub fn init(
        _spi1: pac::SPI1,
        prec: rec::Spi1,
        lcd_pins: LcdPins,
        _clocks: &CoreClocks,
        delay: &mut Delay,
    ) {
        let (_mosi, _miso, _sck, mut cs, mut reset) = (
            lcd_pins.0.into_alternate::<5>(),
            lcd_pins.1.into_alternate::<5>(),
            lcd_pins.2.into_alternate::<5>(),
            lcd_pins
                .3
                .into_push_pull_output_in_state(PinState::High)
                .speed(gpio::Speed::Low),
            lcd_pins
                .4
                .into_push_pull_output_in_state(PinState::High)
                .speed(gpio::Speed::Low),
        );

        // reset LCD
        reset.set_high();
        delay.delay_ms(100_u16);
        reset.set_low();
        delay.delay_ms(100_u16);
        reset.set_high();
        delay.delay_ms(100_u16);

        prec.enable(); // enable peripheral

        // unsafe is unavoidable and ok during initialization of the hardware
        //
        // the details are taken from
        // https://github.com/larus-breeze/sw_frontend/tree/hw_frontend_stm32h743_hal_lqfp100_rgb18_st7701
        unsafe {
            let spi1 = &(*pac::SPI1::ptr());
            spi1.cr1.write(|w| w.bits(0x0000_1000));
            spi1.cfg1.write(|w| w.bits(0x5007_0008));
            spi1.crcpoly.write(|w| w.bits(0x0000_0107));
            spi1.cfg2.write(|w| w.bits(0x4442_0000));
        }

        fn write_data(cs: &mut gpio::Pin<'H', 4, Output>, _delay: &mut Delay, data: &[u16]) {
            for wo in data {
                cs.set_low();

                // unsafe is unavoidable and ok during initialization of the hardware
                unsafe {
                    let spi1 = &(*pac::SPI1::ptr());

                    spi1.cr1
                        .write(|w| w.ssi().slave_not_selected().spe().disabled());

                    spi1.ifcr.write(|w| w.modfc().clear());
                    spi1.cr2.write(|w| w.tsize().bits(1));

                    spi1.cr1
                        .write(|w| w.ssi().slave_not_selected().spe().enabled());
                    while spi1.sr.read().txp().is_full() {}

                    spi1.txdr.write(|w| w.bits(*wo as u32));
                    spi1.cr1.modify(|_, w| w.cstart().set_bit());

                    while spi1.sr.read().eot().bit_is_clear() == true {}
                    while spi1.cr1.read().cstart().is_started() {}
                    spi1.ifcr.write(|w| w.txtfc().clear().eotc().clear());
                }
                cs.set_high();
            }
        }

        write_data(&mut cs, delay, &INIT_SEQ_1);
        delay.delay_ms(120_u16);
        write_data(&mut cs, delay, &INIT_SEQ_2);
        delay.delay_ms(200_u16);
    }
}
