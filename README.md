# IoT Solar dehydrator
a prototype for solar dehydrator that utilize solar power  
the STM32's job is to monitor temperature + humidity, control fan PWM depending on Solar power's voltage, then display them into SSD1315 OLED  
the STM32 also provides a way for user to change some configurations by listening to switches and a rotary encoder.  

ESP8266 will be used to enable internet connectivity for STM32, it will be used to send data into a server (if enabled) and allow remotely control this device.  
