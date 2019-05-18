#![allow(unused_unsafe)]
#![no_main]
#![no_std]

extern crate panic_semihosting;

use stm32f3::stm32f303 as stm;
use stm32f3::stm32f303::{interrupt, Interrupt};
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m_rt::ExceptionFrame;
use cortex_m_semihosting as sh;
use stm32f3xx_hal::prelude::*;
use stm32f3xx_hal::time::MegaHertz;
use core::sync::atomic::AtomicU32;
use core::sync::atomic::Ordering;

#[macro_use]
mod util;

static FREQ: AtomicU32 = AtomicU32::new(0);
static FREQ_SCRATCH: AtomicU32 = AtomicU32::new(0);
static SYSTICK_MS: AtomicU32 = AtomicU32::new(0);
const ORD: Ordering = Ordering::SeqCst;

#[cortex_m_rt::entry]
unsafe fn main() -> ! {
    let pcore = cortex_m::Peripherals::take().unwrap();
    let peri = stm32f3::stm32f303::Peripherals::take().unwrap();
    let mut nvic = pcore.NVIC;
    let mut syst = pcore.SYST;

    // set up system clock to max value reachable by HSI
    let mut flash = peri.FLASH.constrain();
    let mut rcc = peri.RCC.constrain();
    rcc.cfgr = rcc.cfgr.sysclk(MegaHertz(64))
                       .hclk(MegaHertz(64))
                       .pclk1(MegaHertz(32))
                       .pclk2(MegaHertz(32));
    rcc.cfgr.freeze(&mut flash.acr);

    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(6400000-1);
    syst.enable_interrupt();

    // set up GPIO
    modif!(RCC.ahbenr: iopaen = true);
    modif!(GPIOA.moder: moder0 = 0b10);
    modif!(GPIOA.afrl: afrl0 = 1);

    // set up TIM2 for counting pulses
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

    nvic.enable(Interrupt::TIM2);
    modif!(TIM2.cr1: cen = true);
    // modif!(TIM2.dier: cc1ie = true);

    FREQ_SCRATCH.store(0, ORD);

    let mut last_disp = 0;
    syst.clear_current();
    syst.enable_counter();
    loop {
        let cur = SYSTICK_MS.load(ORD);
        if cur > last_disp + 10 {
            sh::hprintln!("{} {}", FREQ.load(ORD), readb!(GPIOA.idr: idr0));
            last_disp = cur;
        }
    }
}

#[interrupt]
unsafe fn TIM2() {
    if readb!(TIM2.sr: cc1if) {
        FREQ_SCRATCH.fetch_add(65536, ORD);
        modif!(TIM2.sr: cc1if = false);
    }
}

#[cortex_m_rt::exception]
unsafe fn SysTick() -> ! {
    SYSTICK_MS.fetch_add(1, ORD);

    FREQ.store(FREQ_SCRATCH.load(ORD) + read!(TIM2.cnt: cnt), ORD);

    // TODO apply prescaler
    // reset counter
    write!(TIM2.cnt: cnt = 1);
    write!(TIM2.cnt: cnt = 0);
    FREQ_SCRATCH.store(0, ORD);
}

#[cortex_m_rt::exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("HardFault at {:#?}", ef);
}

#[cortex_m_rt::exception]
fn DefaultHandler(irqn: i16) {
    panic!("Unhandled exception (IRQn = {})", irqn);
}
