use core::panic::PanicInfo; 
use esp_println::println;

#[inline(never)]
#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    //print some info for the user
    if let Some(location) = _info.location() {
        println!("Panic occured in file {}, at line {}", location.file(), location.line());
    } else {
        println!("Panic Occured");
    }
    println!("Message: {}", _info.message());

    //spin and do nothing
    loop {
    }
}