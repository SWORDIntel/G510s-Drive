#include "tab_lighting.h"
#include "ipc.h"

#include <algorithm>
#include <chrono>
#include <cmath>
#include <condition_variable>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <memory>
#include <mutex>
#include <sstream>
#include <thread>

#include <ftxui/component/component.hpp>
#include <ftxui/component/screen_interactive.hpp>
#include <ftxui/dom/elements.hpp>

namespace {
using namespace ftxui;
using namespace std::chrono_literals;
namespace fs = std::filesystem;

constexpr int palette[] = {
    0xff0000, 0x00ff00, 0x0000ff, 0xffffff, 0x00dc00, 0xdc0000,
    0x0000dc, 0xff8800, 0x00ffff, 0x9900ff, 0xffff00, 0x000000,
    0xff0080, 0x80ff00, 0x0080ff, 0xff8000, 0x8000ff, 0x00ff80,
    0xff8080, 0x80ff80, 0x8080ff, 0xffff80, 0xff80ff, 0x80ffff,
    0x800000, 0x008000, 0x000080, 0x808080, 0xc0c0c0, 0x404040,
    0xff4000, 0x40ff00, 0x0040ff, 0xff00ff,
};

std::string hex_color(int rgb) {
  std::ostringstream out;
  out << '#' << std::hex << std::setw(6) << std::setfill('0') << rgb;
  return out.str();
}

fs::path led_path(const std::string& name) {
  const fs::path root = "/sys/class/leds";
  std::error_code ec;
  if (fs::is_directory(root / name, ec)) return root / name;
  fs::path match;
  for (fs::directory_iterator it(root, ec), end; !ec && it != end; it.increment(ec)) {
    if (it->path().filename().string().rfind(name + "-", 0) != 0 || !it->is_directory(ec)) continue;
    if (!match.empty()) return {};
    match = it->path();
  }
  return ec ? fs::path{} : match;
}

int brightness_of(const fs::path& dir) {
  if (dir.empty()) return -1;
  int value;
  if (std::ifstream(dir / "brightness") >> value && value >= 0 && value <= 255) return value;
  return -1;
}

class Lighting : public ComponentBase {
 public:
  explicit Lighting(ScreenInteractive& screen) : screen_(screen) {
    Components rows, row;
    for (int rgb : palette) {
      auto option = ButtonOption::Ascii();
      option.transform = [rgb](const EntryState& s) {
        auto item = hbox({text("██") | color(Color::RGB(rgb >> 16, (rgb >> 8) & 255, rgb & 255)),
                          text(" " + s.label + " ")});
        return s.focused ? item | inverted : item;
      };
      row.push_back(Button(hex_color(rgb), [this, rgb] { stop_animation(true); mode_ = 0; mode_name_ = "solid"; apply(rgb, level_); }, option));
      if (row.size() == 4) {
        rows.push_back(Container::Horizontal(row));
        row.clear();
      }
    }
    rows.push_back(Container::Horizontal({
        Button("-15", [this] { adjust(-15); }, ButtonOption::Ascii()),
        Button("+15", [this] { adjust(15); }, ButtonOption::Ascii())}));
    rows.push_back(Container::Horizontal({
        Button("Solid", [this] { set_mode(0); }, ButtonOption::Ascii()),
        Button("Pulse", [this] { set_mode(1); }, ButtonOption::Ascii()),
        Button("Random", [this] { set_mode(2); }, ButtonOption::Ascii()),
        Button("Rainbow", [this] { set_mode(3); }, ButtonOption::Ascii())}));
    rows.push_back(Container::Horizontal({
        Button("Range start -", [this] { std::lock_guard<std::mutex> lock(animation_mutex_); range_start_ = std::max(0, range_start_ - 1); }, ButtonOption::Ascii()),
        Button("Range start +", [this] { std::lock_guard<std::mutex> lock(animation_mutex_); range_start_ = std::min(range_end_, range_start_ + 1); }, ButtonOption::Ascii()),
        Button("Range end -", [this] { std::lock_guard<std::mutex> lock(animation_mutex_); range_end_ = std::max(range_start_, range_end_ - 1); }, ButtonOption::Ascii()),
        Button("Range end +", [this] { std::lock_guard<std::mutex> lock(animation_mutex_); range_end_ = std::min(33, range_end_ + 1); }, ButtonOption::Ascii())}));
    rows.push_back(Container::Horizontal({
        Button("Cycle -1s", [this] { std::lock_guard<std::mutex> lock(animation_mutex_); cycle_ms_ = std::max(1000, cycle_ms_ - 1000); }, ButtonOption::Ascii()),
        Button("Cycle +1s", [this] { std::lock_guard<std::mutex> lock(animation_mutex_); cycle_ms_ = std::min(60000, cycle_ms_ + 1000); }, ButtonOption::Ascii()),
        Button("Speed -100ms", [this] { std::lock_guard<std::mutex> lock(animation_mutex_); speed_ms_ = std::max(100, speed_ms_ - 100); }, ButtonOption::Ascii()),
        Button("Speed +100ms", [this] { std::lock_guard<std::mutex> lock(animation_mutex_); speed_ms_ = std::min(5000, speed_ms_ + 100); }, ButtonOption::Ascii())}));
    rows.push_back(Container::Horizontal({
        Button("Select M1", [this] { select_profile(1); }, ButtonOption::Ascii()),
        Button("Select M2", [this] { select_profile(2); }, ButtonOption::Ascii()),
        Button("Select M3", [this] { select_profile(3); }, ButtonOption::Ascii())}));
    Add(Container::Vertical(rows));
  }

  ~Lighting() override {
    stop_animation(true);
    {
      std::lock_guard<std::mutex> lock(mutex_);
      stop_ = true;
    }
    wake_.notify_one();
    if (worker_.joinable()) worker_.join();
  }

  Element OnRender() override {
    if (!worker_.joinable()) worker_ = std::thread([this] { poll_status(); });
    std::string daemon, socket;
    bool refresh;
    {
      std::lock_guard<std::mutex> lock(mutex_);
      daemon = status_;
      socket = socket_;
      refresh = refresh_;
      refresh_ = false;
    }
    if (refresh) read_hardware();
    const auto muted = Color::RGB(157, 164, 174);
    const auto surface = Color::RGB(43, 47, 53);
    auto section = [&](const std::string& title, Elements body) {
      auto content = vbox({text(title) | bold | color(Color::RGB(190, 202, 216)), separator(),
                           vbox(std::move(body))});
      return borderRounded(vbox({text(" "), hbox({text(" "), content, text(" ")}), text(" ")})) |
             bgcolor(surface);
    };
    return vbox({
        text("LIGHTING  /  KEYBOARD BACKLIGHT") | bold | color(Color::RGB(220, 224, 230)),
        text(daemon + "   ·   LCD socket: " + socket) | color(muted),
        text("Active profile: " + profile_ + " (saved for reboot)") | color(muted),
        section("CURRENT OUTPUT", {text("Live RGB: " + (rgb_ < 0 ? std::string("unavailable") : hex_color(rgb_))),
                 text("Brightness: " + (level_ < 0 ? std::string("unavailable") : std::to_string(level_) + "/255")),
                 gauge(std::max(level_, 0) / 255.f)}),
        section("LIGHTING CONTROLS", {ChildAt(0)->Render()}),
        section("ANIMATION & PROFILES", {
                 text("Mode: " + mode_name_ + "   |   palette " + std::to_string(range_start_ + 1) + "-" + std::to_string(range_end_ + 1) + "/34"),
                 text("Cycle: " + std::to_string(cycle_ms_ / 1000) + "s   |   transition: " + std::to_string(speed_ms_) + "ms"),
                 text("Arrows: select   Enter: apply   +/-: brightness   M1/M2/M3: save profile"),
                 text(notice_) | color(muted)}),
    }) | yframe | vscroll_indicator;

  }

  bool OnEvent(Event event) override {
    if (event == Event::Character('+') || event == Event::Character('-')) {
      adjust(event == Event::Character('+') ? 15 : -15);
      return true;
    }
    return ComponentBase::OnEvent(event);
  }

 private:
  const char* mode_label() const { return mode_ == 1 ? "pulse" : mode_ == 2 ? "random" : mode_ == 3 ? "rainbow (random switches)" : "solid"; }

  void stop_animation(bool restore) {
    if (!animation_.joinable()) return;
    {
      std::lock_guard<std::mutex> lock(animation_mutex_);
      animation_stop_ = true;
    }
    animation_wake_.notify_one();
    animation_.join();
    if (restore && saved_rgb_ >= 0 && saved_level_ >= 0) {
      try { IpcClient().set_rgb(saved_rgb_ >> 16, (saved_rgb_ >> 8) & 255, saved_rgb_ & 255, saved_level_); }
      catch (...) { notice_ = "Animation stopped; restoring prior lighting failed"; }
    }
    saved_rgb_ = saved_level_ = -1;
  }

  void set_mode(int mode) {
    stop_animation(true);
    mode_ = mode;
    mode_name_ = mode_label();
    if (mode == 0) { notice_ = "Solid mode; choose a palette color"; return; }
    read_hardware();
    if (rgb_ < 0 || level_ < 0) { mode_ = 0; mode_name_ = "solid"; notice_ = "Animation unavailable: current lighting could not be captured"; return; }
    saved_rgb_ = rgb_; saved_level_ = level_;
    { std::lock_guard<std::mutex> lock(animation_mutex_); animation_stop_ = false; }
    animation_ = std::thread([this] { animate(); });
    notice_ = "Started " + mode_name_ + "; previous lighting will be restored on stop";
  }

  void animate() {
    auto cycle_started = std::chrono::steady_clock::now();
    unsigned seed = static_cast<unsigned>(std::chrono::steady_clock::now().time_since_epoch().count());
    int previous = -1;
    int pulse_color;
    {
      std::lock_guard<std::mutex> lock(animation_mutex_);
      pulse_color = range_start_ + static_cast<int>(rand_r(&seed) % static_cast<unsigned>(range_end_ - range_start_ + 1));
    }
    for (;;) {
      int mode, start, end, cycle_ms, speed_ms;
      {
        std::lock_guard<std::mutex> lock(animation_mutex_);
        if (animation_stop_) return;
        mode = mode_;
        start = range_start_;
        end = range_end_;
        cycle_ms = cycle_ms_;
        speed_ms = speed_ms_;
      }
      int idx = mode == 1 ? pulse_color : start + static_cast<int>(rand_r(&seed) % static_cast<unsigned>(end - start + 1));
      if (mode != 1 && end > start && idx == previous) idx = start + (idx - start + 1) % (end - start + 1);
      previous = idx;
      int brightness = saved_level_;
      if (mode == 1) {
        const auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now() - cycle_started).count();
        if (elapsed >= cycle_ms) cycle_started = std::chrono::steady_clock::now();
        const double phase = static_cast<double>(elapsed % cycle_ms) / cycle_ms;
        brightness = std::clamp(static_cast<int>(saved_level_ * (0.15 + 0.85 * (0.5 + 0.5 * std::sin(phase * 6.283185307)))), 0, 255);
      }
      try { const int rgb = palette[idx]; IpcClient().set_rgb(rgb >> 16, (rgb >> 8) & 255, rgb & 255, brightness); }
      catch (...) { std::lock_guard<std::mutex> lock(animation_mutex_); animation_stop_ = true; }
      screen_.PostEvent(Event::Custom);
      std::unique_lock<std::mutex> lock(animation_mutex_);
      if (animation_wake_.wait_for(lock, std::chrono::milliseconds(speed_ms), [this] { return animation_stop_; })) return;
    }
  }

  void poll_status() {
    for (;;) {
      auto next = std::chrono::steady_clock::now() + 5s;
      std::string label, socket = "unavailable";
      try {
        const auto reply = IpcClient(750ms).status();
        if (!reply.at("ok").get<bool>()) throw std::runtime_error(reply.value("error", "request rejected"));
        const auto& data = reply.at("data");
        socket = data.at("socket").get<std::string>();
        label = "Connected: " + data.at("daemon").get<std::string>() + " " + data.at("version").get<std::string>();
      } catch (const std::exception& e) {
        label = std::string("daemon offline (retry in 5s): ") + e.what();
      }
      std::unique_lock<std::mutex> lock(mutex_);
      if (stop_) return;
      status_ = label;
      socket_ = socket;
      refresh_ = true;
      screen_.PostEvent(Event::Custom);
      if (wake_.wait_until(lock, next, [this] { return stop_; })) return;
    }
  }

  void read_hardware() {
    const auto dir = led_path("g15::kbd_backlight");
    level_ = brightness_of(dir);
    int r, g, b;
    rgb_ = -1;
    if (!dir.empty() && std::ifstream(dir / "multi_intensity") >> r >> g >> b &&
        std::min({r,g,b}) >= 0 && std::max({r,g,b}) <= 255) rgb_ = (r << 16) | (g << 8) | b;
    profile_.clear();
    for (int i = 1; i <= 3; ++i) {
      int value = brightness_of(led_path("g15::macro_preset" + std::to_string(i)));
      if (value < 0) { profile_ = "unavailable"; break; }
      if (value == 1) profile_ += (profile_.empty() ? "M" : ", M") + std::to_string(i);
    }
    if (profile_.empty()) profile_ = "none";
  }

  void adjust(int delta) {
    read_hardware();
    if (rgb_ < 0 || level_ < 0) { notice_ = "Cannot adjust: current lighting unavailable"; return; }
    apply(rgb_, std::clamp(level_ + delta, 0, 255));
  }

  void select_profile(int profile) {
    stop_animation(true);
    mode_ = 0; mode_name_ = "solid";
    const auto reply = IpcClient().request("set_profile", {{"number", profile}});
    if (!reply.at("ok").get<bool>()) {
      notice_ = "Profile switch failed: " + reply.value("error", std::string("request rejected"));
      return;
    }
    const auto number = std::to_string(profile);
    read_hardware();
    notice_ = "Selected M" + number + "; daemon applied and saved for boot";
  }

  void apply(int rgb, int level) {
    try {
      if (level < 0) throw std::runtime_error("current brightness unavailable");
      IpcClient().set_rgb(rgb >> 16, (rgb >> 8) & 255, rgb & 255, level);
      notice_ = "Applied " + hex_color(rgb) + " at " + std::to_string(level);
    } catch (const std::exception& e) {
      notice_ = std::string("Lighting write failed: ") + e.what();
    }
    read_hardware();
  }

  ScreenInteractive& screen_;
  std::mutex animation_mutex_;
  std::condition_variable animation_wake_;
  bool animation_stop_ = false;
  std::thread animation_;
  int saved_rgb_ = -1, saved_level_ = -1;
  int mode_ = 0, range_start_ = 0, range_end_ = 11, cycle_ms_ = 10000, speed_ms_ = 1000;
  std::string mode_name_ = "solid";
  std::mutex mutex_;
  std::condition_variable wake_;
  bool stop_ = false, refresh_ = true;
  std::string status_ = "Connecting to g510d...", socket_ = "unavailable";
  std::thread worker_;
  int rgb_ = -1, level_ = -1;
  std::string profile_, notice_;
};
}  // namespace

ftxui::Component LightingTab(ftxui::ScreenInteractive& screen) {
  return std::make_shared<Lighting>(screen);
}
