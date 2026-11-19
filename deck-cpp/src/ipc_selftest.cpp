#include "ipc.h"

#include <exception>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <string>

namespace {
namespace fs = std::filesystem;

// Match leds.rs: prefer the exact device; never choose between duplicates.
fs::path resolve_led(const std::string& name) {
  const fs::path root = "/sys/class/leds";
  std::error_code ec;
  if (fs::is_directory(root / name, ec)) return root / name;
  fs::path match;
  for (fs::directory_iterator it(root, ec), end; !ec && it != end; it.increment(ec)) {
    if (it->path().filename().string().rfind(name + "-", 0) != 0) continue;
    if (!it->is_directory(ec)) continue;
    if (!match.empty()) return {};
    match = it->path();
  }
  return ec ? fs::path{} : match;
}

std::string live_rgb(const fs::path& led) {
  if (led.empty()) return "unavailable";
  std::ifstream in(led / "multi_intensity");
  std::ostringstream hex;
  hex << '#' << std::hex << std::setfill('0');
  for (int i = 0; i < 3; ++i) {
    int channel;
    if (!(in >> channel) || channel < 0 || channel > 255) return "unavailable";
    hex << std::setw(2) << channel;
  }
  std::string extra;
  return in >> extra ? "unavailable" : hex.str();
}
}  // namespace

int main() {
  try {
    IpcClient client;
    const auto reply = client.status();
    if (!reply.at("ok").get<bool>()) {
      std::cerr << "GetStatus: " << reply.value("error", std::string("request rejected")) << '\n';
      return 1;
    }
    const auto& data = reply.at("data");
    if (!data.is_object()) {
      std::cerr << "GetStatus: expected a status object\n";
      return 1;
    }
    std::cout << "daemon_ipc: " << data.dump() << '\n';
    // GetStatus reports daemon metadata; g510ctl reads the hardware separately.
    std::string profiles;
    bool complete = true;
    for (int n = 1; n <= 3; ++n) {
      const auto preset = resolve_led("g15::macro_preset" + std::to_string(n));
      int on;
      if (preset.empty() || !(std::ifstream(preset / "brightness") >> on)) complete = false;
      else if (on == 1) profiles += (profiles.empty() ? "M" : ",M") + std::to_string(n);
    }
    std::cout << "active_profile: " << (!complete ? "unavailable" : profiles.empty() ? "none" : profiles) << '\n';
    const auto led = resolve_led("g15::kbd_backlight");
    std::cout << "live_rgb: " << live_rgb(led) << '\n';
    int brightness;
    if (!led.empty() && (std::ifstream(led / "brightness") >> brightness) &&
        brightness >= 0 && brightness <= 255) {
      std::cout << "brightness: " << brightness << '\n';
    } else {
      std::cout << "brightness: unavailable\n";
    }
    return 0;
  } catch (const std::exception& e) {
    std::cerr << "GetStatus: " << e.what() << '\n';
    return 1;
  }
}
