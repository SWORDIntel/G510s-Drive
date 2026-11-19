#pragma once

#include <chrono>
#include <cstdint>
#include <string>
#include <nlohmann/json.hpp>

// Synchronous, single-caller client. Requests connect lazily and return the
// full {ok, data?, error?} envelope. Transport/protocol failures throw.
class IpcClient {
public:
    explicit IpcClient(std::chrono::milliseconds timeout = std::chrono::seconds(2));
    ~IpcClient();
    IpcClient(const IpcClient&) = delete;
    IpcClient& operator=(const IpcClient&) = delete;

    void connect();
    void disconnect() noexcept;
    nlohmann::json request(const std::string& verb, nlohmann::json payload = nlohmann::json::object());
    nlohmann::json status();
    nlohmann::json ping();
    nlohmann::json toast(const std::string& line1, const std::string& line2 = {}, std::uint64_t duration_ms = 2500);
    nlohmann::json alert(const std::string& mode = "flash", const std::string& color = "#ff0000",
                         std::uint32_t times = 3, float duration = 2.5f);
    // Local hardware operation via sysfs, like g510ctl rgb; not an IPC verb.
    void set_rgb(std::uint8_t r, std::uint8_t g, std::uint8_t b, std::uint8_t brightness = 255);

private:
    std::chrono::milliseconds timeout_;
    int fd_ = -1;
};
