use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::{peripherals::{RADIO_CLK, RNG, TIMG0, WIFI}, rng::Rng, timer::timg::TimerGroup};
use esp_println::{print, println};
use esp_wifi::{init, wifi::{ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice, WifiState}, EspWifiController};
use embassy_net::Config as EmbassyNetConfig;

//#[macro_use]
use super::mk_static;


pub fn start_wifi<'a>(
    wifi: WIFI, 
    clock: RADIO_CLK , 
    rng: RNG, 
    timg0:  TimerGroup<TIMG0>
) -> (WifiController<'static>, Stack<'static>, Runner<'static, WifiDevice<'static, WifiStaDevice>>) {

    let mut rng = Rng::new(rng);

    let init = &*mk_static::mk_static!(
        EspWifiController<'static>,
        init(
            timg0.timer0, 
            rng.clone(),  
            clock,
        ).unwrap()
    );
    let (wifi_interface, wifi_controller) = esp_wifi::wifi::new_with_mode(&init, wifi, WifiStaDevice).unwrap();
    
    /*
        network stack
     */
    let net_config = EmbassyNetConfig::dhcpv4(Default::default());
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;
    let (stack, runner) = embassy_net::new(
        wifi_interface,
        net_config,
        mk_static::mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed
    );

    (wifi_controller, stack, runner)
}

#[embassy_executor::task]
pub async fn connection(mut controller: WifiController<'static>) {
   // println!("start connection task");
   // println!("Device capabilities: {:?}", controller.capabilities());
    let ssid = env!("SSID");
    let passw = env!("PASSW");
    loop {
        //if connected spin here for ever
        match esp_wifi::wifi::wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        //assemble wifi config and start wifi
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: ssid.try_into().unwrap(),
                password: passw.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start_async().await.unwrap();
            println!("Wifi started!");
        }
        print!("About to connect...");

        //try establishing connection - if fails
        //wait 5 seconds and test again
        match controller.connect_async().await {
            Ok(_) => println!(" Wifi connected!"),
            Err(e) => {
                println!(" Wifi connect failed: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    runner.run().await
}