//! Toggle an LED on/off with a button
//!
//! This assumes that a LED is connected to GPIO3.
//! Additionally this assumes a button connected to GPIO35.
//! On an ESP32C3 development board this is the BOOT button.
//!
//! Depending on your target and the board you are using you should change the pins.
//! If your board doesn't have on-board LEDs don't forget to add an appropriate resistor.

use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::{ InterruptType, PinDriver, Pull };
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::timer::config;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{ BlockingWifi, ClientConfiguration, Configuration, EspWifi };

use crate::button::ButtonEvent;
use crate::channel::Channel;
mod time;
mod led;
mod button;
mod channel;

fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'_>>) -> anyhow::Result<()> {
    let ssid = option_env!("WIFI_SSID").unwrap_or("");
    let password = option_env!("WIFI_PASSWORD").unwrap_or("");

    anyhow::ensure!(
        !ssid.is_empty(),
        "Missing WIFI_SSID. Set it when building, e.g. WIFI_SSID=... WIFI_PASSWORD=... cargo run"
    );

    wifi.set_configuration(
        &Configuration::Client(ClientConfiguration {
            ssid: ssid.try_into().map_err(|_| anyhow::anyhow!("WIFI_SSID is too long"))?,
            password: password
                .try_into()
                .map_err(|_| anyhow::anyhow!("WIFI_PASSWORD is too long"))?,
            ..Default::default()
        })
    )?;

    let mut last_err: Option<anyhow::Error> = None;

    for attempt in 0..=5 {
        let result = (|| -> anyhow::Result<()> {
            wifi.start()?;
            wifi.connect()?;
            wifi.wait_netif_up()?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                return Ok(());
            }
            Err(err) => {
                last_err = Some(err);

                if attempt < 5 {
                    FreeRtos::delay_ms(1000);
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Wi-Fi init failed")))
}

fn main() -> anyhow::Result<()> {
    esp_idf_hal::sys::link_patches();

    let peripherals = Peripherals::take()?;

    let modem = peripherals.modem;
    let mut wifi_wakeup = PinDriver::output(peripherals.pins.gpio6)?;
    wifi_wakeup.set_high()?;
    FreeRtos::delay_ms(100);
    wifi_wakeup.set_low()?;

    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sysloop.clone(), Some(nvs))?, sysloop)?;

    if let Err(err) = connect_wifi(&mut wifi) {
        println!("Wi-Fi init failed: {err}");
        println!(
            "Hint: on ESP32-P4, Wi-Fi uses ESP-Hosted over SDIO and needs a reachable slave module."
        );
        println!("Continuing without Wi-Fi.");
    } else {
        println!("Wi-Fi connected.");
    }

    let mut led = PinDriver::output(peripherals.pins.gpio3)?;
    let mut button = PinDriver::input(peripherals.pins.gpio35, Pull::Up)?;

    button.set_interrupt_type(InterruptType::PosEdge)?;

    led.set_high()?;

    let mut timer_config = config::TimerConfig::default();
    timer_config.clock_source = config::ClockSource::PLLF80M;

    let ticker = time::Ticker::new(&timer_config);

    let button_event: Channel<ButtonEvent> = Channel::new();
    let mut button_task = button::ButtonTask::new(&button, &ticker, button_event.get_sender());

    let mut led_task = led::LedTask::new(led, &ticker, button_event.get_receiver());
    loop {
        button_task.poll();
        led_task.poll();
        FreeRtos::delay_ms(1);
    }
}
