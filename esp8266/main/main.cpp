#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <stdio.h>
#include <cstring>
#include "driver/gpio.h"
#include "esp_event.h"
#include "esp_log.h"
#include "esp_wifi_types.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include <esp_sleep.h>
#include "portmacro.h"
#include "projdefs.h"
#include "wifi.hpp"

#include <driver/spi.h>
#include <driver/hspi_logic_layer.h>
#include <esp_wifi.h>

static const char *TAG = "main";

#define SPI_SLAVE_HANDSHAKE_GPIO                ((gpio_num_t) 4)
#define CS_GPIO                                 ((gpio_num_t) 15)

#define SPI_WRITE_BUFFER_MAX_SIZE               2048
#define SPI_READ_COMMAND_MAX_SIZE               1024
#define SPI_READ_BUFFER_MAX_SIZE                128

#ifdef __cplusplus
extern "C" {
#endif
void app_main(void);
#ifdef __cplusplus
}
#endif

enum SPICommand {
    SCAN_WIFI = 0x0,
    CONNECT = 0x1,
    SEND_CLIMATE = 0x2,
    SLEEP = 0x3,
};

enum class SPIResponse {
    OK = 0x0,
    MALFORMED_COMMAND = 0x1,
    MALFORMED_ARGS = 0x2,
    UNABLE_TO_CONNECT = 0x3,
};

constexpr size_t WIFI_SCAN_LEN = 10;
constexpr size_t SSID_LEN = 33; //1 for null termination
constexpr size_t PASSWORD_LEN = 64; //1 for null termination
constexpr size_t WIFI_SCAN_RESULT_LEN = SSID_LEN + 1; //1 for rssi

static void write_res(SPIResponse flag) {
    uint8_t data[] = {static_cast<uint8_t>(flag)};
    hspi_slave_logic_write_data(data, 1, portMAX_DELAY);
}

static void execute_command(uint8_t command[], size_t len, Wifi& wifi){
    switch (command[0]) {
        case SPICommand::SCAN_WIFI : {
            std::array<wifi_ap_record_t, WIFI_SCAN_LEN> scans;
            uint16_t scans_len = wifi.scan(scans);
            uint8_t parsed_scans[WIFI_SCAN_RESULT_LEN * WIFI_SCAN_LEN];
            std::memset(parsed_scans, 0x0, WIFI_SCAN_RESULT_LEN * WIFI_SCAN_LEN);
            for(int i =0; i < scans_len; i++) {
                size_t current_idx = i * WIFI_SCAN_RESULT_LEN;

                std::memcpy(&parsed_scans[current_idx], scans[i].ssid, SSID_LEN);
                parsed_scans[current_idx + SSID_LEN] = scans[i].rssi;

                #ifdef DEBUG_SCAN_WIFI
                ESP_LOGI(TAG, "[%d] - %s", i, scans[i].ssid);
                for (size_t j = 0; j < WIFI_SCAN_RESULT_LEN; ++j) {
                    printf("%02X ", parsed_scans[current_idx + j]);
                }
                printf("\n");
                #endif
            }
            #ifdef DEBUG_SCAN_WIFI
            ESP_LOGI(TAG, "scan len : %d, total byte : %d", scans_len,(int)scans_len * (int)WIFI_SCAN_RESULT_LEN);
            #endif
            hspi_slave_logic_write_data(parsed_scans, scans_len * WIFI_SCAN_RESULT_LEN, portMAX_DELAY);
            break;
        }
        case SPICommand::CONNECT:
            if(len != SSID_LEN + PASSWORD_LEN + 1) {
                write_res(SPIResponse::MALFORMED_ARGS);
                return;
            }
            write_res(SPIResponse::OK);
            break;
        case SPICommand::SEND_CLIMATE :
            if(len != 3) {
                write_res(SPIResponse::MALFORMED_ARGS);
                return;
            }
            write_res(SPIResponse::OK);
            break;
        case SPICommand::SLEEP :
            // write_res(SPIResponse::OK);
            // esp_deep_sleep_set_rf_option(0);
            ESP_LOGI(TAG, "SLEEPING");
            esp_deep_sleep(0);
            break;
        default :
            write_res(SPIResponse::MALFORMED_COMMAND);
    }
}

static void IRAM_ATTR recv_command(void *arg)
{
    Wifi wifi{};
    uint8_t command[SPI_READ_COMMAND_MAX_SIZE];
    size_t command_cursor = 0;
    memset(command, '\0', SPI_READ_COMMAND_MAX_SIZE);

    static uint8_t read_data[SPI_READ_BUFFER_MAX_SIZE];
    uint32_t read_len = 0;
    while(true) {
        read_len = hspi_slave_logic_read_data(read_data, SPI_READ_BUFFER_MAX_SIZE, 200);
        if(read_len == 0 && command_cursor > 0) {
            execute_command(command, command_cursor, wifi);
            memset(command, 0x0, SPI_READ_COMMAND_MAX_SIZE);
            memset(read_data, 0x0, SPI_READ_BUFFER_MAX_SIZE);
            command_cursor = 0;
        }else {
            for(int i=0;i< read_len;i++){
                command[command_cursor++] = read_data[i];
            }
            memset(read_data, 0x0, SPI_READ_BUFFER_MAX_SIZE);
        }
    }
}

void app_main(void)
{
    spi_config_t spi_config;
    spi_config.interface.val = SPI_DEFAULT_INTERFACE;
    spi_config.intr_enable.val = SPI_SLAVE_DEFAULT_INTR_ENABLE;
    spi_config.mode = SPI_SLAVE_MODE;
    spi_config.event_cb = NULL;
    spi_init(HSPI_HOST, &spi_config);
    hspi_slave_logic_device_create(SPI_SLAVE_HANDSHAKE_GPIO, 1, SPI_WRITE_BUFFER_MAX_SIZE, SPI_READ_BUFFER_MAX_SIZE);

    xTaskCreate(recv_command, "recv_command", 8192, NULL, 5, NULL);
}


