#include <stdio.h>
#include <string.h>
#include "driver/gpio.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_system.h"
#include "esp_log.h"

#include "driver/spi.h"
#include "driver/hspi_logic_layer.h"

static const char *TAG = "spi_slave_example";

#define SPI_SLAVE_HANDSHAKE_GPIO                4
#define CS_GPIO                                 15

#define SPI_WRITE_BUFFER_MAX_SIZE               2048
#define SPI_READ_BUFFER_MAX_SIZE                1024

static void IRAM_ATTR spi_slave_read_master_task(void *arg)
{
    static uint8_t read_data[SPI_READ_BUFFER_MAX_SIZE];
    uint32_t read_len = 0;
    for (;;) {
        read_len = hspi_slave_logic_read_data(read_data, SPI_READ_BUFFER_MAX_SIZE, 1000);
        for(int i=0;i< read_len;i++){
            ESP_LOGI(TAG, "%c", read_data[i]);
        }
        memset(read_data, 0x0, SPI_READ_BUFFER_MAX_SIZE);
    }
}

void app_main(void)
{
    spi_config_t spi_config;
    // Load default interface parameters
    // CS_EN:1, MISO_EN:1, MOSI_EN:1, BYTE_TX_ORDER:1, BYTE_TX_ORDER:1, BIT_RX_ORDER:0, BIT_TX_ORDER:0, CPHA:0, CPOL:0
    spi_config.interface.val = SPI_DEFAULT_INTERFACE;

    // Load default interrupt enable
    // TRANS_DONE: false, WRITE_STATUS: true, READ_STATUS: true, WRITE_BUFFER: true, READ_BUFFER: ture
    spi_config.intr_enable.val = SPI_SLAVE_DEFAULT_INTR_ENABLE;
    // Set SPI to slave mode
    spi_config.mode = SPI_SLAVE_MODE;
    // Register SPI event callback function
    spi_config.event_cb = NULL;
    spi_init(HSPI_HOST, &spi_config);

    hspi_slave_logic_device_create(SPI_SLAVE_HANDSHAKE_GPIO, 1, SPI_WRITE_BUFFER_MAX_SIZE, SPI_READ_BUFFER_MAX_SIZE);

    xTaskCreate(spi_slave_read_master_task, "spi_slave_read_master_task", 2048, NULL, 5, NULL);
}


