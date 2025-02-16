//use embassy_time::{Duration, WithTimeout};
use esp_hal::{handler,ram};
//use esp_println::println;
use super::BUTTON;

#[handler]
#[ram]
pub fn gpio_int_handler() {
    let sender = super::BTN_CHANNEL.sender();
    //test if GPIO9 caused interrupt
    if critical_section::with(|cs| {
        BUTTON
            .borrow_ref_mut(cs)
            .as_mut()
            .unwrap()
            .is_interrupt_set()
    }) {
       // println!("GPIO9 triggered an interrupt");
        matches!(sender.try_send(9), Ok(())); //sender.try_send is usable in non-async fn
    } 

    //clear the interrupt
    critical_section::with(|cs| {
        BUTTON
            .borrow_ref_mut(cs)
            .as_mut()
            .unwrap()
            .clear_interrupt()
    });
}