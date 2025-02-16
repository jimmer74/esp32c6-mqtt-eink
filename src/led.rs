//gpio8 for ws2812 LED

use esp_hal_smartled::SmartLedsAdapter;
use esp_println::println;
use smart_leds::SmartLedsWrite;
use smart_leds::RGB8;
use serde::Deserialize;
use crate::LED_CHANNEL;


#[derive(Debug, Copy, Clone, Deserialize)]
pub struct RGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Into<RGB8> for RGB {
    fn into(self) -> RGB8 {
        RGB8::new(self.r, self.g, self.b)
    }
}

 
#[embassy_executor::task]
pub async fn led_task(
    mut led: SmartLedsAdapter<esp_hal::rmt::Channel<esp_hal::Blocking, 0>, 25>
    
) {

    led.write([RGB8{r:0,g:0,b:0}].into_iter()).unwrap();

    loop {
        let receiver = LED_CHANNEL.receiver();
        let rgb = receiver.receive().await;
        println!("received: {:?} in led_task",rgb);
        let color: RGB8 = RGB8{r: rgb.g, g: rgb.r, b: rgb.b}; //RGB8 seems to be wired up wrong!
        led.write([color].into_iter()).unwrap();
    }
} 



/* pub fn parse_rgb(msg: &str) -> Result<RGB, FromHexError> {
    
    //convert to String<8>
    let msg = str_to_hl_string8(msg);
    //split off leading 0x 
    //    --rewrite to handle 0X and error gracefully
    let nums = match msg.split('x').nth(1) {
        Some(nums) => nums,
        None => return Result::Err(FromHexError::InvalidHexCharacter { c: msg.chars().nth(1).unwrap(), index: 1 })
    };

    let mut rgb = [0u8;3];
    hex::decode_to_slice(nums, &mut rgb)?; //we return early with parse error if it fails

    //else return our RGB value
    Ok(RGB { r: rgb[1], g: rgb[0], b: rgb[2] })
} */

