use esp_idf_hal::gpio::{ Output, PinDriver };

use crate::{ button::ButtonEvent, channel::Receiver, time::Ticker };

enum LedState {
    On,
    Off,
    SlowBlink,
    MediumBlink,
    FastBlink,
}

pub struct LedTask<'a> {
    led: PinDriver<'static, Output>,
    ticker: &'a Ticker<'a>,
    state: LedState,
    receiver: Receiver<'a, ButtonEvent>,
}

impl<'a> LedTask<'a> {
    pub fn new(
        led: PinDriver<'static, Output>,
        ticker: &'a Ticker<'a>,
        receiver: Receiver<'a, ButtonEvent>
    ) -> Self {
        Self {
            led,
            ticker,
            state: LedState::Off,
            receiver,
        }
    }

    pub fn poll(&mut self) {
        match self.state {
            LedState::On => {
                self.led.set_high().unwrap();
                if let Some(event) = self.receiver.receive() {
                    match event {
                        ButtonEvent::Pressed => {
                            println!("LED Off");
                            self.state = LedState::Off;
                        }
                    }
                }
            }
            LedState::Off => {
                self.led.set_low().unwrap();
                if let Some(event) = self.receiver.receive() {
                    match event {
                        ButtonEvent::Pressed => {
                            println!("Slow blink");
                            self.state = LedState::SlowBlink;
                        }
                    }
                }
            }
            LedState::SlowBlink => {
                if self.ticker.now().ticks() % 2_000_000 < 1_000_000 {
                    self.led.set_high().unwrap();
                } else {
                    self.led.set_low().unwrap();
                }
                if let Some(event) = self.receiver.receive() {
                    match event {
                        ButtonEvent::Pressed => {
                            println!("Medium blink");
                            self.state = LedState::MediumBlink;
                        }
                    }
                }
            }
            LedState::MediumBlink => {
                if self.ticker.now().ticks() % 1_000_000 < 500_000 {
                    self.led.set_high().unwrap();
                } else {
                    self.led.set_low().unwrap();
                }
                if let Some(event) = self.receiver.receive() {
                    match event {
                        ButtonEvent::Pressed => {
                            println!("Fast blink");
                            self.state = LedState::FastBlink;
                        }
                    }
                }
            }
            LedState::FastBlink => {
                if self.ticker.now().ticks() % 500_000 < 250_000 {
                    self.led.set_high().unwrap();
                } else {
                    self.led.set_low().unwrap();
                }
                if let Some(event) = self.receiver.receive() {
                    match event {
                        ButtonEvent::Pressed => {
                            println!("LED On");
                            self.state = LedState::On;
                        }
                    }
                }
            }
        }
    }
}
