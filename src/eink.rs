//use core::ops::Deref;

use core::str::FromStr;

use alloc::format;
use display_interface_spi::SPIInterface;
use embassy_futures::select::{select3, Either3};
use embedded_graphics::{mono_font::{MonoFont, MonoTextStyle}, prelude::{Point, Size}, primitives::{Circle, Line, PrimitiveStyle, Rectangle, StyledDrawable}, text::{Text, TextStyle}, Drawable};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::{gpio::{self, AnyPin, Input, Level, Output, Pull}, peripherals::SPI2, spi::master::Spi, spi::master::Config as SpiConfig, time::RateExtU32 };
use esp_println::{print, println};
use heapless::{String, Vec};
use profont::{PROFONT_10_POINT, PROFONT_12_POINT, PROFONT_14_POINT, PROFONT_18_POINT, PROFONT_24_POINT, PROFONT_7_POINT, PROFONT_9_POINT};
use serde:: Deserialize;
use weact_studio_epd::{graphics::{Display, Display290TriColor, DisplayRotation}, DisplayDriver, TriColor, WeActStudio290TriColorDriver};

use crate::{read_ip_addr, read_mqtt_addr,  IP_UP_CHANNEL, MQTT_UP_CHANNEL, MSG_CHANNEL};

#[embassy_executor::task]
pub async fn eink(
    mut driver: DisplayDriver<SPIInterface<ExclusiveDevice<Spi<'static, esp_hal::Async>, gpio::Output<'static>, embassy_time::Delay>, gpio::Output<'static>>, Input<'static>, gpio::Output<'static>, embassy_time::Delay, 128, 128, 296, TriColor>, 
    mut display:  Display<128, 296, 9472, TriColor>){

    //UI Points
    const IP_PT: Point = Point::new(10, 10);
    const IP_ADDR_PT: Point = Point::new(IP_PT.x+30, IP_PT.y);
    const MQTT_PT: Point = Point::new(IP_PT.x,IP_PT.y + 20);
    const MQTT_ADDR_PT: Point = Point::new(MQTT_PT.x+50, MQTT_PT.y);

    const SEP_LINE_Y: i32 = IP_PT.y + 30;
    
    const MSG_TITLE_PT: Point = Point::new(MQTT_PT.x, SEP_LINE_Y + 15);
    const MSG_CONT_PT: Point = Point::new(MSG_TITLE_PT.x,MSG_TITLE_PT.y+20);
    
    //UI Fonts
    const IP_TITLE_FONT: MonoTextStyle<TriColor> = MonoTextStyle::new(&PROFONT_12_POINT, TriColor::Black);
    const IP_ADDR_FONT: MonoTextStyle<TriColor> = MonoTextStyle::new(&PROFONT_12_POINT, TriColor::Red);
    const IP_DEL_FONT: MonoTextStyle<TriColor> = MonoTextStyle::new(&PROFONT_12_POINT, TriColor::White);
    

    const MAX_MSG_WIDTH: usize = 35;

    let ip_up_recv = IP_UP_CHANNEL.receiver();
    let mqtt_up_recv = MQTT_UP_CHANNEL.receiver();
    let mqtt_msg_recv = MSG_CHANNEL.receiver();

    
   // clear display
   display.clear(TriColor::White);

   //clear top and draw top line
/*    let _ = Rectangle::new(Point { x: 0, y: 0 }, Size::new(296, 40 - 1))
            .draw_styled(&PrimitiveStyle::with_fill(TriColor::White), &mut display); */
        
   let _ = Line::new(Point::new(0, SEP_LINE_Y), Point::new(296, SEP_LINE_Y))
        .draw_styled(&PrimitiveStyle::with_stroke(TriColor::Black, 1), &mut display);
        
    //Write IP&Mqqt
    _ = Text::with_text_style("IP:",IP_PT,IP_TITLE_FONT,TextStyle::default()).draw(&mut display);
    _ = Text::with_text_style("MQQT:",MQTT_PT,IP_TITLE_FONT,TextStyle::default()).draw(&mut display);

    //Initially unconnected - so addr/mqqt is None/Unconnected
    _ = Text::with_text_style("None",IP_ADDR_PT,IP_ADDR_FONT,TextStyle::default()).draw(&mut display);
    _ = Text::with_text_style("Unconnected",MQTT_ADDR_PT,IP_ADDR_FONT,TextStyle::default()).draw(&mut display);
   // _ = driver.full_update(&display).await;

   _ = Text::with_text_style("Message:",MSG_TITLE_PT,IP_TITLE_FONT,TextStyle::default()).draw(&mut display);
   _ = Text::with_text_style("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz", MSG_CONT_PT, IP_ADDR_FONT, TextStyle::default()).draw(&mut display);

    loop {
        let fut_a = ip_up_recv.receive();
        let fut_b = mqtt_up_recv.receive();
        let fut_c = mqtt_msg_recv.receive();
        //when network is up - obtain/write IP addr
        match select3(
            fut_a, 
            fut_b, 
            fut_c
        ).await {
            //IP addr change
             Either3::First(ip_up) => {
                //clear prior circle
                _ = Circle::with_center(Point { x: 296-20, y: 20 },15)
                        .draw_styled(&PrimitiveStyle::with_fill(TriColor::White), &mut display);

                _ = Text::with_text_style("None",IP_ADDR_PT,IP_DEL_FONT,TextStyle::default()).draw(&mut display);

                let text = match read_ip_addr() {
                    Some(text) => text,
                    None => String::<21>::from_str("None").unwrap(),     
                };
                _ = Text::with_text_style(&text,IP_ADDR_PT,IP_ADDR_FONT,TextStyle::default()).draw(&mut display);
                if ip_up {
                    _ = Circle::with_center(Point { x: 296-20, y: 20 },15)
                        .draw_styled(&PrimitiveStyle::with_stroke(TriColor::White, 2), &mut display);
                }
                _ = driver.full_update(&mut display).await;


            }, 
            //Mqtt connection change
            Either3::Second(mqtt_up) => {
            /*     _ = Rectangle::new(Point { x: MQTT_ADDR_PT.x - 1, y: MQTT_ADDR_PT.y - 1 }, Size::new(296-MQTT_ADDR_PT.x as u32 + 1, 14))
                    .draw_styled(&PrimitiveStyle::with_fill(TriColor::White),&mut display); */
                _ = Text::with_text_style("None",IP_ADDR_PT,IP_DEL_FONT,TextStyle::default()).draw(&mut display);
                _ = Text::with_text_style("Unconnected!",MQTT_ADDR_PT,IP_DEL_FONT,TextStyle::default()).draw(&mut display);

                let ip_addr = match read_ip_addr() {
                    Some(text) => text,
                    None => String::<21>::from_str("None").unwrap(),     
                };
                let mqtt_addr = match read_mqtt_addr() {
                    Some(text) => format!("{}:{}",text.0, text.1),
                    None => format!("Unconnected!"),
                };

                _ = Text::with_text_style(&ip_addr,IP_ADDR_PT,IP_ADDR_FONT,TextStyle::default()).draw(&mut display);
                _ = Text::with_text_style(&mqtt_addr,MQTT_ADDR_PT,IP_ADDR_FONT,TextStyle::default())
                        .draw(&mut display);

                if mqtt_up {
                    _ = Circle::with_center(Point { x: 296-20, y: 20 },15)
                            .draw_styled(&PrimitiveStyle::with_fill(TriColor::Red), &mut display);
                } else {
                    _ = Circle::with_center(Point { x: 296-20, y: 20 },15)
                            .draw_styled(&PrimitiveStyle::with_fill(TriColor::White), &mut display);
                }

                _ = driver.full_update(&mut display).await;

            },

            //Mqtt msg incoming
            // TODO: replace vec with deque for msg for faster popping of 1st element of msg
            Either3::Third(msg) => {
                println!("received eink bundle");
   
                //draw white rectangle over the "message" area (basically deletes a prior message without having to redraw all the other UI stuff)
                _ = Rectangle::new(
                    Point { x: MSG_CONT_PT.x - 5, y: MSG_CONT_PT.y - 15 }, 
                    Size::new(296 - 5, 80)
                    ).draw_styled(
                        &PrimitiveStyle::with_fill(TriColor::White), 
                        &mut display);

                //decompose the msg into a vector of words for easier text processing
                let mut msg = msg.data.split_ascii_whitespace().collect::<Vec<&str, 30>>();
                
                //vec to hold words from current line and a variable to hold character length of that line (including spaces)
                let mut line_length = 0;
                //TODO: keep this as vec as we are pushing to back, so deque gives no advantage
                let mut line: Vec<&str, 30> = Vec::new();

                //variable to hold y_offset of current line 
                let mut y_offset: i32 = 0;
                
                //loop while msg still has words left in it
                loop {
                    
                    //get char length of current word (the 1st word in msg vector)
                    let word_len = 
                        if let Some(word) = msg.iter().nth(0) {
                            word.len()
                        } else {
                            0
                        };

                    //if current line length plus the next word length is less than width of display
                    //and we haven't already emptied msg, then:
                    // 1) remove the word from msg    
                    // 2) add word to current line
                    // 3) increase current line character length by the word length plus 1 (for the space character)
                    if line_length + word_len < MAX_MSG_WIDTH && !msg.is_empty(){
                        line.push(msg.remove(0)).unwrap();
                        line_length += word_len + 1;
                    //line is finsihed (as in adding more words exceeds display width, or no more words left to process in msg)
                    } else {
                        //add back spaces and collect current line vec into a string
                        let cur_line = line.join(" ");

                        // Draw string at correct y_offset for whatever line number we're on           
                        _ = Text::with_text_style(
                            &cur_line,
                            Point::new(MSG_CONT_PT.x, MSG_CONT_PT.y + y_offset),
                            IP_ADDR_FONT,
                            TextStyle::default()
                                ).draw(&mut display);
                        
                        //if msg is empty, break out of loop to get driver to actually update the screen
                        if msg.is_empty() {
                            break;

                        //msg has words left to process, reset variables for next iteration of loop
                        } else {
                            //reset current line vec and length variables for next loop        
                            line.clear();
                            line_length = 0;
                            //increase y_offset for next line
                            y_offset += 13;
                        }
                    }
               }
         
                _ = driver.full_update(&mut display).await;

            },
        };
 
    } //loop

}


#[derive(Debug, Clone, Deserialize)]
pub struct Msg {
    data: String<300>
}
/* 
#[derive(Debug, Clone,  Deserialize)]
pub struct EINK {
    msg: String::<20>,
    color: MyColor,
    pos: MyPoint,
    fontsize: MyFontSize
} */

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum MyColor {
    Black,
    White,
    Red
}

impl Into<TriColor> for MyColor {
    fn into(self) -> TriColor {
        match self {
            Self::Black => TriColor::Black,
            Self::Red => TriColor::Red,
            Self::White => TriColor::White
        }
    }
} 

impl Into<MonoFont<'static>> for MyFontSize {
    fn into(self) -> MonoFont<'static> {
        match self {
            Self::Pf7pt => PROFONT_7_POINT,
            Self::Pf9pt => PROFONT_9_POINT,
            Self::Pf10pt => PROFONT_10_POINT,
            Self::Pf12pt => PROFONT_12_POINT,
            Self::Pf14pt => PROFONT_14_POINT,
            Self::Pf18pt => PROFONT_18_POINT,
            Self::Pf24pt => PROFONT_24_POINT
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct MyPoint {
    x: i32,
    y: i32
}

impl Into<Point> for MyPoint {
    fn into(self) -> Point {
        Point {
            x: self.x,
            y: self.y
        }
    }
    
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum MyFontSize {
    Pf24pt,
    Pf18pt,
    Pf14pt,
    Pf12pt,
    Pf10pt,
    Pf9pt,
    Pf7pt
}

pub async fn display_init(disp_pins: DispPins) -> (
                    DisplayDriver<
                        SPIInterface<
                            ExclusiveDevice<Spi<'static, esp_hal::Async>, gpio::Output<'static>, embassy_time::Delay>, gpio::Output<'static>
                        >,
                        Input<'static>, 
                        gpio::Output<'static>, 
                        embassy_time::Delay,
                        128,
                        128, 
                        296, 
                        TriColor>, 
                    Display<
                        128, 
                        296, 
                        9472, 
                        TriColor>) {
    let spi_config = SpiConfig::default().with_frequency(10.MHz()).with_mode(esp_hal::spi::Mode::_0).with_write_bit_order(esp_hal::spi::BitOrder::MsbFirst);
    let spi_bus = Spi::new(disp_pins.spi2, spi_config).unwrap().with_sck(disp_pins.sclk).with_mosi(disp_pins.mosi).into_async();

    
   // println!("Intializing SPI Device...");
    let delay = embassy_time::Delay;

    let spi_device =  ExclusiveDevice::new(spi_bus, disp_pins.cs, delay).expect("could not init spi device"); 
    let spi_interface = SPIInterface::new(spi_device, disp_pins.dc);
    
    // Setup EPD
    let delay1 = embassy_time::Delay;
    
    let mut driver = WeActStudio290TriColorDriver::new(spi_interface, disp_pins.busy, disp_pins.rst, delay1);
    
    print!("Intializing EPD...");
    _ = driver.init().await;
    println!(" Done!");
    
    let mut display = Display290TriColor::new();
    display.set_rotation(DisplayRotation::Rotate90);

    (driver, display)
}


pub struct DispPins {
    pub sclk: AnyPin,
    pub mosi: AnyPin,
    pub cs: Output<'static>,
    pub dc: Output<'static>,
    pub rst: Output<'static>,
    pub busy: Input<'static>,
    pub spi2: SPI2
}

impl DispPins {
    pub fn new(sclk: AnyPin, mosi: AnyPin, cs: AnyPin, dc: AnyPin, rst: AnyPin, busy: AnyPin, spi2: SPI2) -> Self {

        Self {
            sclk: sclk,
            mosi: mosi,
            cs: Output::new(cs, Level::High),
            dc: Output::new(dc, Level::Low),
            rst: Output::new(rst, Level::High),
            busy: Input::new(busy, Pull::Up),
            spi2: spi2
        }
    }
}