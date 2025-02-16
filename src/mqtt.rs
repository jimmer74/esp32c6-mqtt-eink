use core::{ops::Add, str::from_utf8, sync::atomic::{AtomicU8, Ordering}};

use alloc::format;
use embassy_futures::select::{select3, Either3};
use embassy_net::{tcp::TcpSocket, Ipv4Address, Stack};
use embassy_time::{Duration, Timer};
use esp_println::println;
use heapless::Vec;
use rust_mqtt::{client::{client::MqttClient, client_config::ClientConfig}, packet::v5::reason_codes::ReasonCode, utils::rng_generator::CountingRng};

use crate::{eink::Msg, led::RGB, write_ip_addr, write_mqtt_addr, BTN_CHANNEL, LED_CHANNEL, MQTT_MAX_BUF_SIZE, MQTT_MAX_QOS, MQTT_PING_TO, MQTT_VER, MSG_CHANNEL};
static MQTT_RETRY_COUNT: AtomicU8 = AtomicU8::new(0);
const MQTT_MAX_RETRY_COUNT: u8 = 5;
/* 
* -------------------------------------------------------------------------------------------------
*
*
*                   Mqqt Messaging Task:
*
*           1) Create a TcpSocket
*           2) Connect/Auth to MqqtBroker
*           3) Subscribe to "test/light" & "test/msg" topics
*           4) Loop/Wait continuously while
*            a) Reacting to incoming topic messages (e.g. setting RGB led or eink display messages)
*            b) Send outgoing "test/switch" messages on interrupt
*            c) sending MqqtPing packets before keepalive timeout 
*               to stop broker closing connection 
*           5) Restart task if various things go wrong to re-establish socket/connection to broker
*
* --------------------------------------------------------------------------------------------------
*/
#[embassy_executor::task]
pub async fn mqtt_task(stack: Stack<'static>) {


'mqqt_setup: loop{
    let current_loop = MQTT_RETRY_COUNT.load(Ordering::Relaxed);

    if current_loop == MQTT_MAX_RETRY_COUNT {
        println!("Reached Max Mqtt connection retries: {}/{}. Giving up...", current_loop, MQTT_MAX_RETRY_COUNT);
        write_mqtt_addr(None);
        break 'mqqt_setup;
    } 
    
    // check link is up - busy-wait if not
    if !stack.is_link_up() {
        Timer::after(Duration::from_millis(500)).await;
        continue 'mqqt_setup;
    }


    // check we got IP address - busy-wait if not
    if let Some(net_config) = stack.config_v4() {
        write_ip_addr(Some(net_config.address));
        println!("Got IP: {}", net_config.address);
       
    } else {
        Timer::after(Duration::from_millis(100)).await;
        continue 'mqqt_setup;

    }
    
    if current_loop > 0 {
        println!("Mqtt restarted {}/{} tries", current_loop, MQTT_MAX_RETRY_COUNT);
    }
    

    //tcp socket buffers
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
 
    //now we are ready to open a tcp socket
    let remote_endpoint = gen_mqtt_remote_endpoint();
    let mut tcp_sock = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);   
    tcp_sock.set_timeout(Some(Duration::from_secs(10))); 
    tcp_sock.set_keep_alive(Some(Duration::from_secs(5)));
    match tcp_sock.connect(remote_endpoint).await {
        Ok(_) => {},
        Err(_e) => {
           // println!("Tcp connection error: {:?}", e);
            Timer::after(Duration::from_millis(1000)).await;
            MQTT_RETRY_COUNT.store( current_loop.add(1), Ordering::Relaxed);
            continue 'mqqt_setup;
        },
    };
  
    let mut mqtt_config = ClientConfig::new(
        MQTT_VER, 
        CountingRng(20000),
    );

    mqtt_config.add_max_subscribe_qos(MQTT_MAX_QOS);
    mqtt_config.add_client_id(env!("MQTT_ID"));
    mqtt_config.add_username(env!("MQTT_USER"));
    mqtt_config.add_password(env!("MQTT_PASS"));
    mqtt_config.max_packet_size = 200;
    mqtt_config.keep_alive = 120;
    let mut recv_buffer = [0; MQTT_MAX_BUF_SIZE];
    let mut write_buffer = [0; MQTT_MAX_BUF_SIZE];


    let mut mqtt_client =  MqttClient::<_, 5, _>::new(
        tcp_sock, 
        &mut write_buffer, MQTT_MAX_BUF_SIZE, 
        &mut recv_buffer, MQTT_MAX_BUF_SIZE, 
        mqtt_config 
    ); 

    match mqtt_client.connect_to_broker().await {
        Ok(()) => {
            write_mqtt_addr(Some(remote_endpoint));
            println!("Connected to MQTT broker at {:?}:{}", env!("MQTT_ADDR"), env!("MQTT_PORT"));
        },
        Err(mqtt_err) => {
            write_mqtt_addr(None);            
            match mqtt_err {
                ReasonCode::NetworkError => println!("MQTT Network Error"),
                _ => println!("Other MQTT Error: {:?}", mqtt_err),
            }
            MQTT_RETRY_COUNT.store( current_loop.add(1), Ordering::Relaxed);
            continue 'mqqt_setup;
        }
    } 

    let mut topics = Vec::<_,4>::new();
    _ = topics.push("test/light");
    _ = topics.push("test/eink");
    match mqtt_client.subscribe_to_topics(&topics).await {
        Ok(()) => println!("Subscribed to topics {:?}", topics),
        Err(e) => {
            println!("Error subbing to test/topic: {}", e)
        },
    };

    loop {

        //tcp_sock.can_recv();
        /* 
        
        either we:

            a) want to receive a message
            b) want to send a message
            c) want to send a keep_alive ping to broker after some timeout MQTT_PING_TO

            by using select to await which ever future happens 1st, we can 
            process our actions appropriately in their respective match arms.

            
        */
        let eink_sender = MSG_CHANNEL.sender();
        let btn_receiver = BTN_CHANNEL.receiver();
        let led_sender  = LED_CHANNEL.sender();
        
        
        //set up inter-task communication channels
        let rec_fut = mqtt_client.receive_message();
        let btn_fut = btn_receiver.receive();
        let tim_fut = Timer::after_secs(MQTT_PING_TO as u64);


        match select3(
                rec_fut, 
                btn_fut, 
                tim_fut
                ).await {

            /*
            * --------------------------------------------------------------
            *
            *
            *           Receiving mqtt message from broker
            *
            * 
            * --------------------------------------------------------------
            */
            Either3::First(msg) => {
                match msg {
                    Ok(msg) => {
                        let (topic, body) = msg;
                        let len = body.len();
                        let msg = from_utf8(body).unwrap();

                        println!("Received Topic: {}, with body len: {}, body: {} ", topic, len, msg);

                        match topic {
                            "test/light" => {
                                match serde_json_core::from_slice::<RGB>(body) {
                                    Ok(msg) => {
                                        led_sender.send(msg.0).await;
                                    },
                                    Err(e) => {
                                        println!("malformed json: {}", e);
                                    }
                                };
                            },
                            "test/eink" => {
                                match serde_json_core::from_slice::<Msg>(body) {
                                    Ok((msg,_)) => {
                                        eink_sender.send(msg).await;
                                    }
                                    Err(e) => {
                                        println!("malformed json: {}", e);
                                    }
                                }
                            }
                            _ => println!("ignoring unknown topic: {}", topic),
                        }
                    },
                    Err(e) => {
                        if e == ReasonCode::UnspecifiedError {
                            led_sender.send(RGB { r: 0, g: 0, b: 0 }).await;
                            MQTT_RETRY_COUNT.store( current_loop.add(1), Ordering::Relaxed);
                            continue 'mqqt_setup;
                        } 
                    }
                }
            },
            /*
            * --------------------------------------------------------------
            *
            *
            *           Sending MQTT message to Broker
            *
            * 
            * --------------------------------------------------------------
            */
            //Sending Message
            Either3::Second(val) => {
                let msg = format!("button {} pressed", val);
                match mqtt_client.send_message(
                    "test/switch", 
                    msg.as_bytes(), 
                    MQTT_MAX_QOS, 
                    false
                    ).await {
                        Ok(()) => {},
                        Err(e) => println!("Mqtt Sending err: {}", e),
                    }
            },
            /*
            * --------------------------------------------------------------
            *
            *
            *                   Pinging MQTT Broker 
            *              (to defeat keep alive timeout)
            *
            * 
            * --------------------------------------------------------------
            */
            Either3::Third(()) => { 
                match mqtt_client.send_ping().await {
                    Ok(()) => {},
                    Err(e) => {
                        println!("Mqtt Pinging err: {}", e);
                        match e {
                            ReasonCode::ImplementationSpecificError => {
                                //Seems to happen if ping is sent and message is incoming
                                //but not fatal, so we ignore and carry on
                            },
                            _ => {
                                //other errors might indicate probalem with socket or
                                //a dropped connection, so we restart 'mqqt_setup loop 
                                //to re-establish socket/connection
                                MQTT_RETRY_COUNT.store( current_loop.add(1), Ordering::Relaxed);
                                continue 'mqqt_setup;

                            }
                        }
                    },
                }
            },
        }
    }
}

loop {
   //busy-wait due to cumulative mqtt errors
   Timer::after_millis(500).await;  
}
}



pub fn gen_mqtt_remote_endpoint() -> (Ipv4Address, u16) {
    let addr = env!("MQTT_ADDR");
    let mqtt_port = env!("MQTT_PORT").parse::<u16>().unwrap();
    let addr: Vec<u8, 4> = addr.split(".").map(|x| x.parse::<u8>().unwrap()).collect();
    let mqtt_addr = Ipv4Address::new(addr[0], addr[1], addr[2], addr[3]);
    
    (mqtt_addr,mqtt_port )
} 
    
      
