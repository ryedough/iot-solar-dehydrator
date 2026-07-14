#pragma once
#include "esp_log.h"
#include "esp_wifi.h"
#include "esp_wifi_types.h"
#include <array>
#include <cstddef>
#include <FreeRTOS.h>
#include <freertos/event_groups.h>

class Wifi {
public:
    Wifi();
    template <std::size_t LEN>
    uint16_t scan(std::array<wifi_ap_record_t, LEN>& ssid_list);
    void connect(const char* ssid, const char* password);
};

template <std::size_t LEN>
uint16_t Wifi::scan(std::array<wifi_ap_record_t, LEN>& ssid_list){

    wifi_scan_config_t scan_config = {
        .ssid = 0,
        .bssid = 0,
        .channel = 0,
        .show_hidden = true,
        .scan_type = WIFI_SCAN_TYPE_ACTIVE,
        .scan_time={
            .active = wifi_active_scan_time_t{
                .min = 500,
                .max = 1500,
            }
        }
    };
    ESP_LOGI("Wifi", "Starting Wi-Fi scan...");
    ESP_ERROR_CHECK(esp_wifi_scan_start(&scan_config, true));

    uint16_t len = ssid_list.size();
    ESP_ERROR_CHECK(esp_wifi_scan_get_ap_records(&len,ssid_list.data()));
    ESP_ERROR_CHECK(esp_wifi_scan_stop());
    return len;
}
