use super::CLUT_COLORS;
use stm32h7xx_hal::{
    gpio::{self, Speed},
    pac,
    rcc::{rec, CoreClocks, ResetEnable},
};

pub struct LtdcPins(
    pub gpio::Pin<'A', 5>,  // R4
    pub gpio::Pin<'A', 8>,  // R6
    pub gpio::Pin<'C', 0>,  // R5
    pub gpio::Pin<'D', 3>,  // G7
    pub gpio::Pin<'G', 6>,  // R7
    pub gpio::Pin<'G', 7>,  // PCLK
    pub gpio::Pin<'G', 10>, // B2
    pub gpio::Pin<'I', 11>, // G6
    pub gpio::Pin<'I', 12>, // HSYNC
    pub gpio::Pin<'I', 13>, // VSYNC
    pub gpio::Pin<'J', 1>,  // R2
    pub gpio::Pin<'J', 2>,  // R3
    pub gpio::Pin<'J', 9>,  // G2
    pub gpio::Pin<'J', 11>, // G4
    pub gpio::Pin<'J', 12>, // G3
    pub gpio::Pin<'J', 15>, // B3
    pub gpio::Pin<'K', 0>,  // G5
    pub gpio::Pin<'K', 3>,  // B4
    pub gpio::Pin<'K', 4>,  // B5
    pub gpio::Pin<'K', 5>,  // B6
    pub gpio::Pin<'K', 6>,  // B7
    pub gpio::Pin<'K', 7>,  // DE
);

/// Panel timing in pixel clocks (horizontal) and lines (vertical)
struct PanelTiming {
    hsync: u16,
    hbp: u16,
    hfp: u16,
    vsync: u16,
    vbp: u16,
    vfp: u16,
}

// Small display (series): the values that have always been used here
// (528 x 510 clocks per frame, 9 MHz -> approx. 33.5 Hz).
#[cfg(not(feature = "display_large"))]
const TIMING: PanelTiming = PanelTiming {
    hsync: 6,
    hbp: 18,
    hfp: 24,
    vsync: 4,
    vbp: 10,
    vfp: 16,
};

/// LTDC pixel clock (pll3_r_ck, set in init.rs)
#[cfg(not(feature = "display_large"))]
pub const PIXEL_CLOCK_HZ: u32 = 9_000_000;

// Large display (2.76", Z28016): porches as given by the panel manufacturer (porch.jpg,
// 788 x 528 clocks per frame). The manufacturer runs it with 25 MHz / 60 Hz; 14 MHz gives
// approx. 33-34 Hz, the same frame rate as the small display, so the VSYNC synchronized
// drawing (one frame per LTDC line interrupt) behaves exactly as before.
// The ST7701S needs at least 672 clocks per line with this init code (C2h, RTNI = 0x0A).
#[cfg(feature = "display_large")]
const TIMING: PanelTiming = PanelTiming {
    hsync: 8,
    hbp: 160,
    hfp: 140,
    vsync: 8,
    vbp: 20,
    vfp: 20,
};

/// LTDC pixel clock (pll3_r_ck, set in init.rs)
#[cfg(feature = "display_large")]
pub const PIXEL_CLOCK_HZ: u32 = 14_000_000;

const ACTIVE_W: u16 = 480;
const ACTIVE_H: u16 = 480;

// LTDC register values, all accumulated and minus one as the reference manual wants them
const HSW: u16 = TIMING.hsync - 1;
const AHBP: u16 = TIMING.hsync + TIMING.hbp - 1;
const AAW: u16 = AHBP + ACTIVE_W;
const TOTALW: u16 = AAW + TIMING.hfp;
const VSH: u16 = TIMING.vsync - 1;
const AVBP: u16 = TIMING.vsync + TIMING.vbp - 1;
const AAH: u16 = AVBP + ACTIVE_H;
const TOTALH: u16 = AAH + TIMING.vfp;
/// line interrupt two lines after the active area (start of the vertical blanking)
const LINE_IRQ_POS: u16 = AAH + 2;
/// layer window: first and last active pixel / line
const LAYER_WHPCR: u32 = ((AAW as u32) << 16) | (AHBP as u32 + 1);
const LAYER_WVPCR: u32 = ((AAH as u32) << 16) | (AVBP as u32 + 1);

// The small display must keep exactly the register values it always had
#[cfg(not(feature = "display_large"))]
const _: () = assert!(
    HSW == 5
        && AHBP == 0x17
        && AAW == 0x01f7
        && TOTALW == 0x020f
        && VSH == 3
        && AVBP == 0x0d
        && AAH == 0x01ed
        && TOTALH == 0x01fd
        && LINE_IRQ_POS == 0x01ef
        && LAYER_WHPCR == 0x01f7_0018
        && LAYER_WVPCR == 0x01ed_000e
);

// ST7701S with the large display's init code: at least 512 + 10 * 16 clocks per line
#[cfg(feature = "display_large")]
const _: () = assert!(TOTALW + 1 >= 672);

pub struct Ltdc {}

impl Ltdc {
    pub fn init(
        _ltdc: pac::LTDC,
        ltdc_pins: LtdcPins,
        prec: rec::Ltdc,
        clocks: &CoreClocks,
    ) -> Self {
        // The pll3_r_ck (== pixel clock, PIXEL_CLOCK_HZ) leads to a frame rate of slightly
        // more than 30 Hz for both display variants
        let _ = clocks.pll3_r_ck().unwrap(); // pll3 must run
        prec.enable().reset(); // enable peripheral

        ltdc_pins
            .0
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .1
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .2
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .3
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .4
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .5
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .6
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .7
            .into_alternate::<9>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .8
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .9
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .10
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .11
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .12
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .13
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .14
            .into_alternate::<9>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .15
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .16
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .17
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .18
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .19
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .20
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);
        ltdc_pins
            .21
            .into_alternate::<14>()
            .speed(Speed::Low)
            .internal_pull_up(true);

        // unsafe is unavoidable and ok during initialization of the hardware
        //
        // the details are taken from
        // https://github.com/larus-breeze/sw_frontend/tree/hw_frontend_stm32h743_hal_lqfp100_rgb18_st7701
        unsafe {
            // configure ltdc peripheral
            let ltdc = &(*pac::LTDC::ptr());
            ltdc.sscr.write(|w| w.vsh().bits(VSH));
            ltdc.sscr.modify(|_, w| w.hsw().bits(HSW));

            ltdc.bpcr.write(|w| w.avbp().bits(AVBP));
            ltdc.bpcr.modify(|_, w| w.ahbp().bits(AHBP));

            ltdc.awcr.write(|w| w.aah().bits(AAH));
            ltdc.awcr.modify(|_, w| w.aaw().bits(AAW));

            ltdc.twcr.write(|w| w.totalh().bits(TOTALH));
            ltdc.twcr.modify(|_, w| w.totalw().bits(TOTALW));

            ltdc.ier
                .modify(|_, w| w.terrie().set_bit().fuie().set_bit());

            // Line interrupt two lines after the active area: the vertical
            // blanking reload has just happened, so the buffer that was on
            // screen until now is free for drawing. The view task is spawned
            // from this interrupt (VSYNC synchronized rendering).
            ltdc.lipcr.write(|w| w.lipos().bits(LINE_IRQ_POS));
            ltdc.ier.modify(|_, w| w.lie().set_bit());

            ltdc.gcr.write(|w| w.pcpol().set_bit());

            Ltdc {}
        }
    }

    pub fn init_layer(&mut self, frame_bauffer: *const u8) {
        unsafe {
            // write color lookup table
            let ltdc = &(*pac::LTDC::ptr());
            for clut_entry in CLUT_COLORS {
                ltdc.layer1.clutwr.write(|w| w.bits(clut_entry));
            }
            ltdc.layer1.cr.modify(|_, w| w.cluten().enabled()); // enable clut
            ltdc.srcr.write(|w| w.imr().set_bit());

            // configure the layer used
            ltdc.layer1.whpcr.write(|w| w.bits(LAYER_WHPCR));
            ltdc.layer1.wvpcr.write(|w| w.bits(LAYER_WVPCR));
            ltdc.layer1.pfcr.write(|w| w.bits(0x05));
            ltdc.layer1.bfcr.write(|w| w.bits(0x0000_0405));
            ltdc.layer1.cfblr.write(|w| w.bits(0x01e0_01e7));
            ltdc.layer1.cfblnr.write(|w| w.bits(0x0000_01e0));

            ltdc.layer1
                .cfbar
                .write(|w| w.cfbadd().bits(frame_bauffer as u32));
            ltdc.layer1.cr.modify(|_, w| w.len().enabled()); // enable layer 1
            ltdc.srcr.write(|w| w.imr().set_bit());

            // activate ltdc peripheral
            ltdc.gcr.modify(|_, w| w.ltdcen().set_bit()); // enable ltdc
        }
    }

    pub fn set_frame_buffer(&mut self, frame_bauffer: *const u8) {
        unsafe {
            let ltdc = &(*pac::LTDC::ptr());
            ltdc.layer1
                .cfbar
                .write(|w| w.cfbadd().bits(frame_bauffer as u32));
            ltdc.srcr.write(|w| w.vbr().set_bit());
        }
    }

    /// Clear the line interrupt flag (called from the LTDC interrupt)
    pub fn clear_line_irq() {
        unsafe {
            let ltdc = &(*pac::LTDC::ptr());
            ltdc.icr.write(|w| w.clif().set_bit());
        }
    }

    /// A vertical blanking reload is still pending: the buffer handed out
    /// for drawing is still on screen, drawing into it now would tear
    pub fn reload_pending() -> bool {
        unsafe {
            let ltdc = &(*pac::LTDC::ptr());
            ltdc.srcr.read().vbr().bit_is_set()
        }
    }

    /// Wait until the shadow reload requested above has taken place.
    ///
    /// Until the vertical blanking reload happens, the LTDC still displays the
    /// buffer that is handed out for drawing. Without this wait the drawing of
    /// the next frame (clear, menu, clear, editor) becomes visible as flicker.
    /// The bounded loop makes sure we never hang if the LTDC is not running.
    pub fn wait_reload(&self) {
        unsafe {
            let ltdc = &(*pac::LTDC::ptr());
            let mut guard = 4_000_000_u32;
            while ltdc.srcr.read().vbr().bit_is_set() && guard > 0 {
                guard -= 1;
                core::hint::spin_loop();
            }
        }
    }
}
