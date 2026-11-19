#include "ipc.h"

#include <cerrno>
#include <climits>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <poll.h>
#include <stdexcept>
#include <system_error>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

namespace {
namespace fs = std::filesystem;
using Clock = std::chrono::steady_clock;

std::string socket_path() {
    if (const char* env = std::getenv("G510_SOCKET")) {
        std::string s(env);
        constexpr char whitespace[] = " \t\n\r\f\v";
        auto first = s.find_first_not_of(whitespace);
        if (first != std::string::npos)
            return s.substr(first, s.find_last_not_of(whitespace) - first + 1);
    }
    auto dir = fs::path("/run/user") / std::to_string(getuid());
    std::error_code ec;
    return fs::is_directory(dir, ec) ? (dir / "g510d.sock").string() : "/tmp/g510d.sock";
}

void wait_ready(int fd, short event, Clock::time_point deadline) {
    pollfd p{fd, event, 0};
    for (;;) {
        auto ms = std::chrono::ceil<std::chrono::milliseconds>(deadline - Clock::now()).count();
        if (ms <= 0) throw std::runtime_error("g510d IPC request timed out");
        int n = ::poll(&p, 1, static_cast<int>(ms));
        if (n > 0) return; // recv/send will report EOF or the socket error.
        if (n < 0 && errno != EINTR) throw std::system_error(errno, std::generic_category(), "poll g510d");
    }
}

fs::path live_led() {
    const fs::path root = "/sys/class/leds";
    const std::string name = "g15::kbd_backlight";
    if (fs::is_directory(root / name)) return root / name;
    fs::path match;
    for (const auto& entry : fs::directory_iterator(root)) {
        if (entry.path().filename().string().rfind(name + "-", 0) != 0 || !entry.is_directory()) continue;
        if (!match.empty()) throw std::runtime_error("ambiguous sysfs LED: " + name);
        match = entry.path();
    }
    if (match.empty()) throw std::runtime_error("sysfs LED unavailable: " + name);
    return match;
}

void write_attr(const fs::path& path, const std::string& value) {
    std::ofstream out(path);
    out << value << '\n';
    out.close();
    if (!out) throw std::runtime_error("failed to write " + path.string());
}
} // namespace

IpcClient::IpcClient(std::chrono::milliseconds timeout) : timeout_(timeout) {
    if (timeout.count() <= 0 || timeout.count() > INT_MAX)
        throw std::invalid_argument("invalid IPC timeout");
}

IpcClient::~IpcClient() { disconnect(); }

void IpcClient::disconnect() noexcept {
    if (fd_ >= 0) ::close(fd_);
    fd_ = -1;
}

void IpcClient::connect() {
    if (fd_ >= 0) return;
    std::string path = socket_path();
    sockaddr_un addr{};
    addr.sun_family = AF_UNIX;
    if (path.size() >= sizeof(addr.sun_path)) throw std::system_error(ENAMETOOLONG, std::generic_category(), path);
    std::memcpy(addr.sun_path, path.c_str(), path.size() + 1);
    fd_ = ::socket(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    if (fd_ < 0) throw std::system_error(errno, std::generic_category(), "socket");
    // Linux AF_UNIX connects immediately, or returns EAGAIN if the queue is full.
    if (::connect(fd_, reinterpret_cast<sockaddr*>(&addr), sizeof(addr)) < 0) {
        int err = errno;
        disconnect();
        throw std::system_error(err, std::generic_category(), "connect " + path);
    }
}

nlohmann::json IpcClient::request(const std::string& verb, nlohmann::json payload) {
    if (!payload.is_object()) throw std::invalid_argument("IPC payload must be an object");
    payload["cmd"] = verb;
    const auto wire = payload.dump() + '\n';
    connect();
    const auto deadline = Clock::now() + timeout_;
    try {
        for (std::size_t pos = 0; pos < wire.size();) {
            wait_ready(fd_, POLLOUT, deadline);
            auto n = ::send(fd_, wire.data() + pos, wire.size() - pos, MSG_NOSIGNAL);
            if (n < 0 && (errno == EAGAIN || errno == EINTR)) continue;
            if (n <= 0) throw std::system_error(n ? errno : EPIPE, std::generic_category(), "send g510d");
            pos += n;
        }
        std::string line;
        char buf[4096];
        for (;;) {
            wait_ready(fd_, POLLIN, deadline);
            auto n = ::recv(fd_, buf, sizeof(buf), 0);
            if (n < 0 && (errno == EAGAIN || errno == EINTR)) continue;
            if (n < 0) throw std::system_error(errno, std::generic_category(), "recv g510d");
            if (!n) throw std::runtime_error("g510d closed socket before response newline");
            line.append(buf, n);
            if (line.size() > 4 * 1024 * 1024) throw std::runtime_error("g510d response too large");
            auto end = std::memchr(buf, '\n', n);
            if (!end) continue;
            if (end != buf + n - 1) throw std::runtime_error("unexpected bytes after g510d response");
            auto response = nlohmann::json::parse(line, nullptr, false);
            if (!response.contains("ok") || !response["ok"].is_boolean())
                throw std::runtime_error("invalid g510d JSON response: expected object with boolean ok");
            return response;
        }
    } catch (...) {
        disconnect(); // Never reuse a partial frame or replay a side effect.
        throw;
    }
}

nlohmann::json IpcClient::status() { return request("get_status"); }
nlohmann::json IpcClient::ping() { return request("ping"); }

nlohmann::json IpcClient::toast(const std::string& line1, const std::string& line2, std::uint64_t duration_ms) {
    return request("toast", {{"message", line2.empty() ? line1 : line1 + '\n' + line2}, {"duration_ms", duration_ms}});
}

nlohmann::json IpcClient::alert(const std::string& mode, const std::string& color, std::uint32_t times, float duration) {
    return request("alert", {{"mode", mode}, {"color", color}, {"times", times}, {"duration", duration}});
}

void IpcClient::set_rgb(std::uint8_t r, std::uint8_t g, std::uint8_t b, std::uint8_t brightness) {
    auto led = live_led();
    write_attr(led / "multi_intensity", std::to_string(r) + " " + std::to_string(g) + " " + std::to_string(b));
    write_attr(led / "brightness", std::to_string(brightness));
}
