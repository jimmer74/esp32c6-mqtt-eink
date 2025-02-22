# Waveshare ESP32-C6 Zero Demo

**A basic tech demo for this waveshare board - written in Rust using embassy**

1. Utilises a Weact 2.9" Tricolour e-ink display to display messages (and current IP and Mqqt broker connection status) received as part of a mqtt json payload
2. Allows driving and setting of the on-board WS2812 neopixel led via a mqtt json payload
3. Uses the on-board "boot" button to send a mqtt payload to the broker

## Rational

Have done some esp-idf-hal projects (using std) previously (and other nostd embassy projects with the nrf52), so wanted to do something to check out esp-hal/embassy with one of these c6 chips. Eventually with an eye to doing something with zigbee when rust support matures. In the meantime, thought WiFi/Mqtt would be a fun leg-stretching exercise, so here this is.

It's not ground-breaking, does absolutely nothing useful, but does demonstrate what the chip's capable of and what can be done with embassy nowadays with relatively little effort. I also got to check out one of these £5 weact eink displays from aliexpress, then first time I've ever done anything with e-ink.

I don't normally make stuff I've written public, but thought this might help someone else just getting their feet wet with embedded rust/embassy async.

Don't at me for my Rust coding style - like Moana I'm self-taught. 🤣

I also like to split things up into their own modules. This includes the embassy tasks associated with something (ie mqtt messaging or the eink display) along with their helper functions. Helps me navigate the code more easily, I know some people like all their tasks in one place though...

I also wanted to actually finish a project (including documentation and a case) and will upload the stl files once I have something that is working/the correct size/3d printable.

## Tricolor Eink Warning!

Please note refresh rate on Tricolour einks is horrendously slow - three different coloured balls must be jiggled in the z-axis to present the correct colour at the pixel in question. Refresh times of up to 10-15 secs are not unheard of. There is no fast refresh on Tricolours like there is on the black and white ones. Something to consider when choosing these displays! Accroding to something I read on adafruit once (don't have a link - sorry) you should refresh these things a maximum of 1 time every 3 minutes to avoid damaging the display. Not sure how true that is, but something to also bear in mind

## Project Notes

### Connections

The neopixel (GPIO8) and the button (GPIO9) are using onboard devices, so if you're using the same board, you won't need to change those assignments. Obviously, if you're using something else, you'll need to change those things in main.

The Weact 2.9" I have on the following pins:

* SCLK: GPIO15
* MOSI: GPIO14
* CS: GPIO2
* DC: GPIO3
* RST: GPIO4
* BUSY: GPIO5

As far as I'm aware you can only use SPI2 on this chip, and the pin assignment for the module is whatever you want, but this is what I did and have tested it works. Equally, other pin assignments may be fine...

### Compiling and running

ssid/wifi password/mqqt server settings must be suplied at compile-time either:

1. Supplied in .cargo/config.toml
2. Supplied as Env variables on command line:

   ```bash
   SSID="WIFI_SSID" PASSW="WIFI_PASSWORD" MQTT_PORT="1883" MQTT_USER="USER" MQTT_PASS="PASSWORD" MQTT_ADDR="192.168.1.X" cargo build
   ```

Obviously it's easier to supply them in config.toml, but then you have to make sure that file is either in your .gitignore or you do a

```bash
git update-index --skip-worktree .cargo/config.toml
```

*after* you've added your passwords but *before* you upload anything to a public repo. (Don't want peeps getting their hands on your passwords!)

To build, flash and monitor in one step you can run:

```bash
cargo run && espmonitor /dev/MY_USB_MODEM_PORT
```

**cargo run** uses espflash, so you need to make sure you have those tools (espflash/espmonitor) installed for this convenience command to work.

You'll be greated with:

```
..snip..
I (303) boot: Loaded app from partition at offset 0x10000
I (306) boot: Disabling RNG early entropy source...
Intializing EPD... Done!
Starting wifi
Wifi started!
About to connect... Wifi connected!
Got IP: 192.168.0.206/16
Connected to MQTT broker at "192.168.0.7":1883
Subscribed to topics ["test/light", "test/eink"]

```

And the screen should show:

![1740060804288](image/README/1740060804288.jpg)

All looks ok, but the profont £ leaves a lot to be desired!

### Button Interrupts

For testing purposes you can listen to

```bash
mosquitto_sub -L "mqtt://USER:PASSWORD@MQTT_BROKER_IP/test/switch"
```

And you'll be rewarded with:

`button 9 pressed`

everytime you press the onboard boot button. Exciting!

### EINK MQTT Messages

You can publish:

```bash
mosquitto_pub -L "mqtt://USER:PASSWORD@MQTT_BROKER_IP/test/eink" -m '{"data": "Testing that this display can work with long sentences that span multiple lines of text and display a nice paragraph."}' -q 1 --nodelay
```

result:

![1740061108601](image/README/1740061108601.jpg)

Nice!

To delete the message from the screen, just send an empty data string:

```bash
mosquitto_pub -L "mqtt://USER:PASSWORD@MQTT_BROKER_IP/test/eink" -m '{"data": ""}' -q 1 --nodelay
```

Et Voila:

![1740062310305](image/README/1740062310305.jpg)

### Neopixel MQTT Messages

You can publish:

```bash
mosquitto_pub -L "mqtt://USER:PASSWORD@MQTT_BROKER_IP/test/light" -m '{"r": 50, "g": 0, "b": 0}' -q 1 --nodelay
```

And red:

![1740062204534](image/README/1740062204534.jpg)

or green

```bash
mosquitto_pub -L "mqtt://USER:PASSWORD@MQTT_BROKER_IP/test/light" -m '{"r": 0, "g": 50, "b": 0}'
```

![1740062216328](image/README/1740062216328.jpg)

or blue

```bash
mosquitto_pub -L "mqtt://USER:PASSWORD@MQTT_BROKER_IP/test/light" -m '{"r": 0, "g": 0, "b": 50}'
```

![1740062227035](image/README/1740062227035.jpg)

or any combo (from 0 to 255 for each colour):

```bash
mosquitto_pub -L "mqtt://USER:PASSWORD@MQTT_BROKER_IP/test/light" -m '{"r": 30, "g": 5, "b": 70}'
```

![1740062236875](image/README/1740062236875.jpg)

or turn off again:

```bash
mosquitto_pub -L "mqtt://USER:PASSWORD@MQTT_BROKER_IP/test/light" -m '{"r": 0, "g": 0, "b": 0}'
```

![1740062270604](image/README/1740062270604.jpg)
