#![no_main]
#![no_std]
#![allow(deprecated)]

extern crate panic_semihosting;

use core::fmt::{self, Write};
use stm32f3::stm32f303 as stm;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m_rt::ExceptionFrame;
use stm32f3xx_hal::prelude::*;
use stm32f3xx_hal::time::{Bps, MegaHertz};
use stm32f3xx_hal::serial::Serial;
use heapless::consts::U4;
use heapless::spsc;

#[macro_use]
mod util;

static mut Q: spsc::Queue<u32, U4, u8> = spsc::Queue::u8();

struct Writer<P>(Serial<stm::USART2, P>);

impl<P> fmt::Write for Writer<P> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            write!(USART2.tdr: tdr = b as u16);
            wait_for!(USART2.isr: txe);
        }
        Ok(())
    }
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let pcore = cortex_m::Peripherals::take().unwrap();
    let peri = stm::Peripherals::take().unwrap();
    let mut syst = pcore.SYST;

    // set up system clock to max value reachable by HSI
    let mut flash = peri.FLASH.constrain();
    let mut rcc = peri.RCC.constrain();
    rcc.cfgr = rcc.cfgr.sysclk(MegaHertz(64))
                       .hclk(MegaHertz(64))
                       .pclk1(MegaHertz(32))
                       .pclk2(MegaHertz(32));
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let mut gpioa = peri.GPIOA.split(&mut rcc.ahb);
    let gpiob = peri.GPIOB.split(&mut rcc.ahb);
    let pa2 = gpioa.pa2.into_af7(&mut gpioa.moder, &mut gpioa.afrl);
    let pa3 = gpioa.pa3.into_af7(&mut gpioa.moder, &mut gpioa.afrl);
    let mut pa5 = gpioa.pa5.into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
    let mut out = Writer(Serial::usart2(peri.USART2, (pa2, pa3), Bps(115200),
                                        clocks, &mut rcc.apb1));

    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(16_000_000-1);  // every 0.25s
    syst.enable_interrupt();

    // set up external timer input (XXX can't do this with the hal crate)
    modif!(GPIOA.moder: moder0 = 0b10);
    modif!(GPIOA.afrl: afrl0 = 1);

    // set up TIM2 for counting pulses (can't do it with hal either)
    modif!(RCC.apb1enr: tim2en = true);
    pulse!(RCC.apb1rstr: tim2rst);

    write!(TIM2.arr: arr = 65535);
    write!(TIM2.smcr:
           sms = 0b111,  // external clock mode 1
           etf = 0b0000, // no filtering for now
           etp = false,  // polarity = rising
           etps = 0b00,  // no prescaler
           ts = 0b111    // external trigger input
    );

    modif!(TIM2.cr1: urs = true);
    modif!(TIM2.cr1: cen = true);

    syst.clear_current();
    syst.enable_counter();
    let mut results = unsafe { Q.split().1 };
    let mut toggle = false;

    let disp_mode = gpiob.pb7.is_high();
    if disp_mode {
        let _ = core::write!(out, "\x1b\x1b\x02\x40\x00");             // clear to black
        let _ = core::write!(out, "\x1b\x1b\x03\x30\x30\x30");         // set position
        let _ = core::write!(out, "\x1b\x1b\x02\x31\x02");             // set font
        let _ = core::write!(out, "\x1b\x1b\x05\x33\x00\x08\x07\x0f"); // set colors
        let _ = core::write!(out, "\x1b\x1b\x01\x20");                 // switch to gfx mode
    }

    loop {
        cortex_m::asm::wfi(); // let systick fire, no busy-wait
        if let Some(freq) = results.dequeue() {
            toggle = !toggle;
            if toggle { pa5.set_high(); } else { pa5.set_low(); }
            if disp_mode {
                core::write!(out, "\x1b\x1b\x0b\x44{:7} Hz", freq).unwrap();
            } else {
                core::write!(out, "\r{:7} Hz", freq).unwrap();
            }
        }
    }
}

#[cortex_m_rt::exception]
#[allow(non_upper_case_globals, unused_unsafe)]
unsafe fn SysTick() -> ! {
    static mut n: u32 = 0;
    *n += 1;
    if *n == 4 {
        let _ = Q.split().0.enqueue(read!(TIM2.cnt: cnt));
        write!(TIM2.cnt: cnt = 0);
        *n = 0;
    }
}

#[cortex_m_rt::exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("HardFault at {:#?}", ef);
}

#[cortex_m_rt::exception]
fn DefaultHandler(irqn: i16) {
    panic!("Unhandled exception (IRQn = {})", irqn);
}
