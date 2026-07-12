#include <stdbool.h>
#include <driver/i2c.h>
#include <esp_log.h>
#include <stdint.h>
#include <sys/types.h>
#include "esp_err.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_system.h"
#include "esp_spi_flash.h"
#define I2C_NUM I2C_NUM_0
#define I2C_SCL_IO           5                /*!< gpio number for I2C master clock */
#define I2C_SDA_IO           4               /*!< gpio number for I2C master data  */
#define SSD_1315_ADDR 0x3C

const char* TAG = "main";

static esp_err_t ssd1315_write_cmd(uint8_t* cmd_data, size_t len){
    esp_err_t ret;
    i2c_cmd_handle_t cmd = i2c_cmd_link_create();
    i2c_master_start(cmd);
    i2c_master_write_byte(cmd, (SSD_1315_ADDR << 1) | I2C_MASTER_WRITE, true);
    for(size_t i=0; i<len; i++) {
        i2c_master_write_byte(cmd, 0x2 << 6, true); // control byte
        i2c_master_write_byte(cmd, cmd_data[i], true); // data byte
    }
    i2c_master_stop(cmd);
    ret = i2c_master_cmd_begin(I2C_NUM, cmd, 1000/portTICK_RATE_MS);
    i2c_cmd_link_delete(cmd);

    return ret;
}

static esp_err_t ssd1315_write_data(uint8_t* data, size_t len){
    esp_err_t ret;
    i2c_cmd_handle_t cmd = i2c_cmd_link_create();
    i2c_master_start(cmd);
    i2c_master_write_byte(cmd, SSD_1315_ADDR << 1 | I2C_MASTER_WRITE, true);
    for(size_t i=0; i<len; i++) {
        i2c_master_write_byte(cmd, 0x3 << 6, true); // control byte
        i2c_master_write_byte(cmd, data[i], true); // data byte
    }
    i2c_master_stop(cmd);
    ret = i2c_master_cmd_begin(I2C_NUM, cmd, 1000/portTICK_RATE_MS);
    i2c_cmd_link_delete(cmd);

    return ret;
}

static void install_i2c(){
    i2c_config_t conf;
    conf.mode = I2C_MODE_MASTER;
    conf.sda_io_num = I2C_SDA_IO;
    conf.sda_pullup_en = 1;
    conf.scl_io_num = I2C_SCL_IO;
    conf.scl_pullup_en = 1;
    conf.clk_stretch_tick = 300; // 300 ticks, Clock stretch is about 210us, you can make changes according to the actual situation.
    ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM, conf.mode));
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM, &conf));

    //hack, my esp i2c wont work in the very first command
    i2c_cmd_handle_t cmd = i2c_cmd_link_create();
    i2c_master_start(cmd);
    i2c_master_write_byte(cmd, SSD_1315_ADDR << 1 | I2C_MASTER_WRITE, true);
    i2c_master_stop(cmd);
    i2c_master_cmd_begin(I2C_NUM, cmd, 1000/portTICK_RATE_MS);
    i2c_cmd_link_delete(cmd);
}

static esp_err_t ssd1315_init()
{
    install_i2c();
    uint8_t cmds[] = {
        0xA8, 0x3F, // Set Mux Ratio
        0xD3, 0x00, // Set Display offset
        0x20, 0x00, // Set Adressing mode to vertical
        0x40,       // Set start line
        0xA1,       // Set segment re-map / 0xA0
        0xC8,       // Set COM output scan direction / 0xC0
        0xDA, 0x12, // Set COM pin hardware configuration
        0x81, 0x7F, // Set contrast
        0xA4,       // Resume the display
        0xD5, 0x80, // Set Oscillator frequency
        0x8D, 0x14, // Enable Charge pump
        0xAF        // Turn the display on
    };
    return ssd1315_write_cmd(cmds, sizeof(cmds));
}

void ssd1315_set_page_addr(uint8_t start, uint8_t end) {
    uint8_t cmd[]= {
        0x22,
        start & 0x07,
        end & 0x07,
    };
    ssd1315_write_cmd(cmd, sizeof(cmd));
}

void ssd1315_set_col_addr(uint8_t start, uint8_t end) {
    uint8_t cmd[]= {
        0x21,
        start,
        end,
    };
    ssd1315_write_cmd(cmd, sizeof(cmd));
}

typedef enum {
    ON,
    OFF
}FramebufferPixelState;

bool ssd1315_set(uint8_t buf[1024], uint8_t row, uint8_t col, FramebufferPixelState state){
    uint8_t mapped_row = row/8;
    short unsigned int target_id = mapped_row * 128 + col;
    uint8_t* const target = &buf[target_id];
    if(row > 63) return false;
    if(col > 127) return false;
    if(state == ON){
        switch(row % 8) {
            case 0 : *target |= 0b00000001; break;
            case 1 : *target |= 0b00000010; break;
            case 2 : *target |= 0b00000100; break;
            case 3 : *target |= 0b00001000; break;
            case 4 : *target |= 0b00010000; break;
            case 5 : *target |= 0b00100000; break;
            case 6 : *target |= 0b01000000; break;
            case 7 : *target |= 0b10000000; break;
        }
    } else if(state == OFF){
        switch(row % 8) {
            case 0 : *target &= 0b11111110; break;
            case 1 : *target &= 0b11111101; break;
            case 2 : *target &= 0b11111011; break;
            case 3 : *target &= 0b11110111; break;
            case 4 : *target &= 0b11101111; break;
            case 5 : *target &= 0b11011111; break;
            case 6 : *target &= 0b10111111; break;
            case 7 : *target &= 0b01111111; break;
        }
    }
    return true;
}

void ssd1315_draw_rect(uint8_t buf[1024], uint8_t x, uint8_t y, uint8_t w, uint8_t h) {
    uint8_t x_end = x + w;
    uint8_t y_end = y + h;

    for(uint8_t yy = y; yy<y_end; yy++){
        for(uint8_t xx = x; xx<x_end; xx++) {
            ssd1315_set(buf, yy, xx, ON);
        }
    }
}

static void ssd1315_task(void* arg){
    ESP_ERROR_CHECK(ssd1315_init());
    while (1) {
        uint8_t framebuffer[1024] = {0};
        ssd1315_draw_rect(framebuffer, 54, 26, 10, 10);
        // ssd1315_set(framebuffer, 63, 127, ON);

        ssd1315_set_col_addr(0, 127);
        ssd1315_set_page_addr(0, 7);

        esp_err_t ret = ssd1315_write_data(framebuffer, sizeof(framebuffer));
        if(ret == ESP_OK){
            ESP_LOGI(TAG, "ack fill data");
        }
        vTaskDelay(10000/ portTICK_RATE_MS);
    }
}


void app_main()
{
    xTaskCreate(ssd1315_task, "ssd1315_task", 2048, NULL, 10, NULL);
    // xTaskCreate(i2c_scanner, "ssd1315_task", 2048, NULL, 10, NULL);
}
