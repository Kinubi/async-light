//! Toggle an LED on/off with a button
//!
//! This assumes that a LED is connected to GPIO3.
//! Additionally this assumes a button connected to GPIO35.
//! On an ESP32C3 development board this is the BOOT button.
//!
//! Depending on your target and the board you are using you should change the pins.
//! If your board doesn't have on-board LEDs don't forget to add an appropriate resistor.

use core::num::NonZero;
use std::cell::Cell;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::{ InterruptType, PinDriver, Pull };
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::task::notification::Notification;
use esp_idf_hal::timer::{ TimerDriver, config, config::TimerConfig };
use fugit::{ Duration, ExtU64 };

use crate::button::{ ButtonEvent, ButtonTask };
use crate::channel::Channel;
use crate::time::TickDuration;
mod time;
mod led;
mod button;
mod channel;

fn main() -> anyhow::Result<()> {
    esp_idf_hal::sys::link_patches();

    let peripherals = Peripherals::take()?;
    let mut led = PinDriver::output(peripherals.pins.gpio3)?;
    let mut button = PinDriver::input(peripherals.pins.gpio35, Pull::Up)?;

    button.set_interrupt_type(InterruptType::PosEdge)?;

    let mut led_state = true;
    led.set_high()?;

    let mut timer_config = config::TimerConfig::default();
    timer_config.clock_source = config::ClockSource::PLLF80M;

    let ticker = time::Ticker::new(&timer_config);

    let mut button_event: Channel<ButtonEvent> = Channel::new();
    let mut button_task = button::ButtonTask::new(&button, &ticker, button_event.get_sender());

    let mut led_task = led::LedTask::new(led, &ticker, button_event.get_receiver());
    loop {
        button_task.poll();
        led_task.poll();
        FreeRtos::delay_ms(1);
        // // prepare communication channel
        // let notification = Notification::new();
        // let waker = notification.notifier();

        // // register interrupt callback, here it's a closure on stack
        // unsafe {
        //     button
        //         .subscribe_nonstatic(move || {
        //             waker.notify(NonZero::new(1).unwrap());
        //         })
        //         .unwrap();
        // }

        // // enable interrupt, will be automatically disabled after being triggered
        // button.enable_interrupt()?;
        // // block until notified
        // notification.wait_any();
        // println!("Button pressed! Toggling LED...");

        // // toggle the LED
        // if led_state {
        //     led.set_low()?;
        //     led_state = false;
        // } else {
        //     led.set_high()?;
        //     led_state = true;
        // }

        // // debounce
        // FreeRtos::delay_ms(200);
        // println!("{}", ticker.now());
    }
}
