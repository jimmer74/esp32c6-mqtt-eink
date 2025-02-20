#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]
#![feature(ascii_char)]


use alloc::format;
use critical_section::Mutex;
use eink::{ display_init, eink, DispPins, Msg};
use embassy_net::{Ipv4Cidr, Ipv4Address};
use esp_alloc as _;
use esp_hal::{
    clock::CpuClock, 
    gpio::{self, Input, Io, Pin}, 
    interrupt::InterruptConfigurable, 
    rmt::Rmt, 
    time::RateExtU32 
};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embassy_sync::{channel::Channel, blocking_mutex::raw::CriticalSectionRawMutex};
use heapless::String;
use core::cell::RefCell;
use lazy_static::lazy_static;
//smartled rgb onboard
use esp_hal_smartled::{smartLedBuffer, SmartLedsAdapter};
use rust_mqtt::{
    client::client_config::MqttVersion, 
    packet::v5::publish_packet::QualityOfService
};
extern crate alloc;

//local modules
mod input; use input::gpio_int_handler;
mod panic;
mod wireless; use wireless::*;
mod mqtt;
mod led; use led::*;
mod eink;
mod mk_static;


/* 
* ----------------------------------------------------------------------
*
*
*                               Statics
*
*
* ----------------------------------------------------------------------
*/


lazy_static! {
    static ref BTN_CHANNEL: Channel<CriticalSectionRawMutex, u8, 1>
        = embassy_sync::channel::Channel::new();
}
lazy_static! {
    static ref LED_CHANNEL: Channel<CriticalSectionRawMutex, RGB, 3>
        = embassy_sync::channel::Channel::new();
}
lazy_static! {
    static ref MSG_CHANNEL: Channel<CriticalSectionRawMutex, Msg, 1>
        = embassy_sync::channel::Channel::new();
}
lazy_static! {
    static ref IP_UP_CHANNEL: Channel<CriticalSectionRawMutex, bool, 1>= embassy_sync::channel::Channel::new();
}

lazy_static! {
    static ref MQTT_UP_CHANNEL: Channel<CriticalSectionRawMutex, bool, 1>= embassy_sync::channel::Channel::new();
}

static BUTTON: Mutex<RefCell<Option<Input>>> = Mutex::new(RefCell::new(None));
static IP_ADDR: Mutex<RefCell<Option<String<21>>>> = Mutex::new(RefCell::new(None));
static MQTT_ADDR: Mutex<RefCell<Option<(String<18>, String<4>)>>> = Mutex::new(RefCell::new(None));
static MQTT_PING_TO: u8 = 30;

const MQTT_VER: MqttVersion = MqttVersion::MQTTv5;
const MQTT_MAX_BUF_SIZE: usize = 240;
const MQTT_MAX_QOS: QualityOfService = QualityOfService::QoS1;

fn write_ip_addr(addr: Option<Ipv4Cidr>) {
   // let sender = IP_UP_CHANNEL.sender();
    match addr {
        Some(addr) => {
            critical_section::with(|cs| {
            let mut st = String::<21>::new();
            _ = st.push_str(&format!("{}/{}", addr.address(), addr.prefix_len()));
            *IP_ADDR.borrow_ref_mut(cs) = Some(st);
            //sender.try_send(true).unwrap();
            });
        }
        None => {
            critical_section::with(|cs| {
                *IP_ADDR.borrow_ref_mut(cs) = None;
              //  sender.try_send(false).unwrap();
            });
        }       
    }
}

fn read_ip_addr() -> Option<String::<21>> {
    let mut addr: Option<String::<21>> = None;
    
    critical_section::with(|cs| {
        addr = IP_ADDR.borrow_ref(cs).clone()

    });

    addr
}

fn write_mqtt_addr(addr: Option<(Ipv4Address, u16)>) {
    let sender = MQTT_UP_CHANNEL.sender();

    match addr {
        Some((addr, port)) => {
            critical_section::with(|cs| {
                //assemble addr
                let mut addr_st = String::<18>::new();
                _ = addr_st.push_str(&format!("{}", addr));
                //assemble port
                let mut port_st = String::<4>::new();
                _ = port_st.push_str(&format!("{}", port));
        
                *MQTT_ADDR.borrow_ref_mut(cs) = Some((addr_st, port_st));
                sender.try_send(true).unwrap();
            });
        }
        None => {
            critical_section::with(|cs| {
                *MQTT_ADDR.borrow_ref_mut(cs) = None;
                sender.try_send(false).unwrap();
            });
        }
        
    }
}

fn read_mqtt_addr() -> Option<(String::<18>, String::<4>)> {
    
    let mut addr: Option<(String::<18>, String::<4>)> = None;
    
    critical_section::with(|cs| {
        addr = MQTT_ADDR.borrow_ref(cs).clone()

    });

    addr
}
/* 
* ----------------------------------------------------------
*
*
*                       Main Task:
*
*           peripheral setup and sub task spawning
*
*
* ----------------------------------------------------------
*/
#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    
    /* 
    * --------------------------------------------------
    *
    *             Peripheral setup boilerplate
    *
    * --------------------------------------------------
    */
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    
    esp_alloc::heap_allocator!(72 * 1024);

    let systimer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(systimer.alarm0);

    /* 
    * --------------------------------------------------
    *
    *          GPIO input and Interrupt set-up
    *
    * --------------------------------------------------
    */

    //set up the onboard boot_sel button as an input
    let mut btn = Input::new(peripherals.GPIO9, gpio::Pull::Up);
    
    //set a generic gpio interrupt handler 
    let mut io = Io::new(peripherals.IO_MUX);
    io.set_interrupt_handler(gpio_int_handler);
    
    //puts our input into a static with a falling edge listener.
    //we have designated input::gpio_int_handler as our handler (and tagged fn with #[interrupt])
    //now we can access BUTTON in a cs in the handler to react to events and clear interrupts
    critical_section::with(|cs| {
        btn.listen(gpio::Event::FallingEdge);
        BUTTON.borrow_ref_mut(cs).replace(btn);
    });    
    
    /* 
    * --------------------------------------------------
    *
    *               RGB Rmt Led Set-up
    *
    * --------------------------------------------------
    */
    let rmt = Rmt::new(peripherals.RMT, 80.MHz() ).unwrap();
    let rmt_buffer = smartLedBuffer!(1);
    let led: SmartLedsAdapter<esp_hal::rmt::Channel<esp_hal::Blocking, 0>, 25> = SmartLedsAdapter::new(rmt.channel0, peripherals.GPIO8, rmt_buffer);

    /* 
    * --------------------------------------------------
    *
    *               eInk Display Setup
    *
    * --------------------------------------------------
    */

    let sclk = peripherals.GPIO15;
    let mosi = peripherals.GPIO14;
    let cs = peripherals.GPIO2;
    let dc = peripherals.GPIO3;
    let rst = peripherals.GPIO4;
    let busy = peripherals.GPIO5;
    let spi2 = peripherals.SPI2;
    let disp_pins = DispPins::new(sclk.degrade(), mosi.degrade(), cs.degrade(), dc.degrade(), rst.degrade(), busy.degrade(), spi2);

    let (driver, display) = display_init(disp_pins).await;
    /* 
    * ----------------------------------------------------------------------
    *
    *                      Wifi hardware setup:
    *
    * ----------------------------------------------------------------------
    */
    let timg0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    let (wifi_controller, stack, runner) = 
                start_wifi(
                    peripherals.WIFI, 
                    peripherals.RADIO_CLK, 
                    peripherals.RNG, 
                    timg0
                );
    
    /* 
    * ----------------------------------------------------
    *
    *                   Task spawning:
    *
    * ----------------------------------------------------
    */

    spawner.spawn(eink(driver, display)).ok();
    Timer::after_secs(2).await;
    spawner.spawn(wireless::connection(wifi_controller)).ok();
    spawner.spawn(wireless::net_task(runner)).ok();
    spawner.spawn(mqtt::mqtt_task(stack)).ok();
    spawner.spawn(led_task(led)).ok();
    
    /* 
    * ----------------------------------------------------
    *
    *                   Busy Loop
    *
    * ----------------------------------------------------
    */
    loop {
        Timer::after(Duration::from_millis(1000)).await;
    }
    
}






  /*   
  // Encode an MQTT Connect packet.
  let mut buf = [0u8;1024];
  let pkt = Packet::Connect(Connect { protocol: Protocol::MQTT311,
                                    keep_alive: 30,
                                    client_id: "esp32c6".into(),
                                    clean_session: true,
                                    last_will: None,
                                    username: Some(env!("MQTT_USER")),
                                    password: Some(env!("MQTT_PASS").as_bytes()) });
       encode_slice(&pkt, &mut buf).unwrap();

       assert_eq!(&buf[14..21], "esp32c6".as_bytes());
       match tcp_sock.sock.write(&mut buf).await {
           Ok(x) => {println!("len sent: {}, buf len: {}", x, buf.len())},
           Err(e) => {println!("Error sending pkt: {:?}", e)}
        };

    let msg = Packet::Publish(Publish { 
            dup: false, 
            qospid: QosPid::AtMostOnce, 
            retain: false, 
            topic_name: "test/topic", 
            payload: "ello from esp".as_bytes() 
        });

        encode_slice(&msg, &mut buf).unwrap();

        match tcp_sock.sock.write(&mut buf).await {
            Ok(x) => {println!("len sent: {}, buf len: {}", x, buf.len())},
            Err(e) => {println!("Error sending pkt: {:?}", e)}
         }; 
    //tcp_sock.sock.send_queue();
    if !mqtt_failed {
        println!("Generating mqqt config:");
        let mut mqtt_config = ClientConfig::new(
            rust_mqtt::client::client_config::MqttVersion::MQTTv5, 
            CountingRng(20000),
        );
    
        mqtt_config.add_max_subscribe_qos(rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS1);
        mqtt_config.add_client_id(env!("MQTT_ID"));
        mqtt_config.add_username(env!("MQTT_USER"));
        mqtt_config.add_password(env!("MQTT_PASS"));
        mqtt_config.max_packet_size = 100;
    
        println!("id: {:?}. user: {:?}, pass: {:?}", mqtt_config.client_id, mqtt_config.username, mqtt_config.password);
        
        println!("Starting mqtt client");
        let mut mqtt_client= MqttClient::<stack,5,_>::new(
                        tcp_sock.sock, 
                        &mut mqtt_bufs.mqtt_send, 80, 
                        &mut mqtt_bufs.mqtt_recv, 80, 
                        mqtt_config
        );


      //  let mut mqtt_client = MqttConn::new(tcp_sock, mqtt_bufs);

        
        match mqtt_client.connect_to_broker().await {
            Ok(()) => {
                println!("Connected to MQTT broker at {:?}, {}", env!("MQTT_ADDR"), env!("MQTT_PORT"));
                
                // client.conn.subscribe_to_topic("test/topic").await.unwrap();
                //  state = MqqtState::Idle;
                
            },
            Err(mqtt_err) => match mqtt_err {
                ReasonCode::NetworkError => {
                    println!("MQTT Network Error");
                
                },
                _ => {
                    println!("Other MQTT Error: {:?}", mqtt_err);
                    
                }          
            },
        };
            
        
       // spawner.spawn(mqtt::mqtt_connection(mqtt_client)).ok();
    } else {
        println!("Not starting mqtt client");
    }
 */


     
            /* let mut buf = [0u8;1024];
            let pkt = Packet::Connect(Connect { protocol: Protocol::MQTT311,
                                            keep_alive: 5,
                                            client_id: env!("MQTT_ID").into(),
                                            clean_session: true,
                                            last_will: None,
                                            username: Some(env!("MQTT_USER")),
                                            password: Some(env!("MQTT_PASS").as_bytes()) });
               encode_slice(&pkt, &mut buf).unwrap();

                       _ = tcp_sock.flush().await;

               let mut pos = 0;
             //  use embassy_io::asynch::Write;
   
               tcp_sock.wait_write_ready().await;
    loop {
    
           //assert_eq!(&buf[14..21], "esp32c6".as_bytes());
           match tcp_sock.write(&mut buf).await {
                Ok(0) => {
                    println!("packet EOF");
                    break;
                },
               Ok(x) => {
                   pos += x;
                   println!("len sent: {}, buf len: {}, buf pos: {}", x, buf.len(), pos);
                },
               Err(e) => {println!("Error sending pkt: {:?}", e)}
            };

    } */

    

