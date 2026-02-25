use esp_idf_hal::gpio::{ Input, PinDriver };
use fugit::ExtU64;

use crate::{ channel::Sender, time::{ Ticker, Timer } };

#[derive(Debug, Clone, Copy)]
pub enum ButtonEvent {
    Pressed,
}

enum ButtonState<'a> {
    WaitingForPress,
    Debouce(Timer<'a>),
}

pub struct ButtonTask<'a> {
    button: &'a PinDriver<'static, Input>,
    ticker: &'a Ticker<'a>,
    state: ButtonState<'a>,
    sender: Sender<'a, ButtonEvent>,
}

impl<'a> ButtonTask<'a> {
    pub fn new(
        button: &'a PinDriver<'static, Input>,
        ticker: &'a Ticker<'a>,
        sender: Sender<'a, ButtonEvent>
    ) -> Self {
        Self {
            button,
            ticker,
            state: ButtonState::WaitingForPress,
            sender,
        }
    }

    pub fn poll(&mut self) {
        match self.state {
            ButtonState::WaitingForPress => {
                if self.button.is_low() {
                    self.state = ButtonState::Debouce(
                        Timer::new(
                            (200).millis(), // 200ms debounce time
                            self.ticker
                        )
                    );
                }
            }
            ButtonState::Debouce(ref timer) => {
                if timer.is_ready() && self.button.is_high() {
                    self.state = ButtonState::WaitingForPress;
                    println!("Button pressed!");
                    self.sender.send(ButtonEvent::Pressed);
                }
            }
        }
    }
}
