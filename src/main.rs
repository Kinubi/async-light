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
use esp_idf_hal::modem::Modem;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::sys;
use esp_idf_hal::task::notification::Notification;
use esp_idf_hal::timer::{ TimerDriver, config, config::TimerConfig };
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{ AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi };
use fugit::{ Duration, ExtU64 };

use crate::button::{ ButtonEvent, ButtonTask };
use crate::channel::Channel;
use crate::time::TickDuration;
mod time;
mod led;
mod button;
mod channel;

fn dump_gpio_map() {
    println!("==== GPIO map dump (ESP-IDF) ====");

    let valid_mask = sys::SOC_GPIO_VALID_GPIO_MASK;
    for gpio in 0..sys::gpio_num_t_GPIO_NUM_MAX {
        if (valid_mask & (1u64 << gpio)) == 0 {
            continue;
        }

        let mut cfg = sys::gpio_io_config_t::default();
        let result = unsafe { sys::gpio_get_io_config(gpio as sys::gpio_num_t, &mut cfg) };

        if result == 0 {
            println!(
                "GPIO{gpio:02}: fun_sel={} sig_out={} ie={} oe={} pu={} pd={} od={} drv={}",
                cfg.fun_sel,
                cfg.sig_out,
                cfg.ie,
                cfg.oe,
                cfg.pu,
                cfg.pd,
                cfg.od,
                cfg.drv
            );
        } else {
            println!("GPIO{gpio:02}: gpio_get_io_config failed (esp_err_t={result})");
        }
    }

    println!("==== end GPIO map dump ====");
}

fn connect_wifi(modem: Modem) -> anyhow::Result<()> {
    let ssid = option_env!("WIFI_SSID").unwrap_or("");
    let password = option_env!("WIFI_PASSWORD").unwrap_or("");

    anyhow::ensure!(
        !ssid.is_empty(),
        "Missing WIFI_SSID. Set it when building, e.g. WIFI_SSID=... WIFI_PASSWORD=... cargo run"
    );

    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sysloop.clone(), Some(nvs))?, sysloop)?;

    let auth_method = if password.is_empty() { AuthMethod::None } else { AuthMethod::WPA2Personal };

    wifi.set_configuration(
        &Configuration::Client(ClientConfiguration {
            ssid: ssid.try_into().map_err(|_| anyhow::anyhow!("WIFI_SSID is too long"))?,
            password: password
                .try_into()
                .map_err(|_| anyhow::anyhow!("WIFI_PASSWORD is too long"))?,
            auth_method,
            ..Default::default()
        })
    )?;

    let mut last_err: Option<anyhow::Error> = None;

    for attempt in 0..=3 {
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

                if attempt < 3 {
                    println!("Wi-Fi/SDIO init retry {}/3 in 1s...", attempt + 1);
                    let _ = wifi.disconnect();
                    let _ = wifi.stop();
                    FreeRtos::delay_ms(1000);
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Wi-Fi init failed")))
}

fn main() -> anyhow::Result<()> {
    esp_idf_hal::sys::link_patches();
    dump_gpio_map();

    let peripherals = Peripherals::take()?;

    let modem = peripherals.modem;
    let mut wifi_wakeup = PinDriver::output(peripherals.pins.gpio6)?;
    wifi_wakeup.set_low()?;

    if let Err(err) = connect_wifi(modem) {
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
