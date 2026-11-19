#include "tab_audio.h"

#include <cstdio>
#include <memory>
#include <string>
#include <sys/wait.h>

#include <ftxui/component/component.hpp>
#include <ftxui/dom/elements.hpp>

namespace {
// Only fixed command literals below are passed to the shell.
std::string query(const char* command, bool status) {
  FILE* pipe = popen(command, "r");
  if (!pipe) return "Unavailable: could not start playerctl.";
  char line[64]{};
  const bool got_line = std::fgets(line, sizeof(line), pipe) != nullptr;
  const int result = pclose(pipe);
  if (result == -1 || !WIFEXITED(result)) return "Unavailable: playerctl interrupted.";
  switch (WEXITSTATUS(result)) {
    case 0: break;
    case 127: return "Unavailable: install playerctl.";
    case 124: return "Unavailable: player did not respond.";
    default: return "Unavailable: start an MPRIS player in this desktop session.";
  }
  if (!status) return {};
  std::string value = got_line ? line : "";
  value.erase(value.find_last_not_of("\r\n") + 1);
  if (value == "Playing" || value == "Paused" || value == "Stopped") return value;
  return "Unavailable: unexpected player status.";
}

std::string playback_status() {
  return query("timeout -k 1s 1s playerctl status 2>/dev/null", true);
}
}  // namespace

ftxui::Component AudioTab() {
  using namespace ftxui;
  auto status = std::make_shared<std::string>();
  auto action = [status](const char* command) {
    *status = query(command, false);
    if (status->empty()) *status = playback_status();
  };
  auto controls = Container::Horizontal({
      Button("Play/Pause", [action] { action("timeout -k 1s 1s playerctl play-pause >/dev/null 2>&1"); }),
      Button("Next", [action] { action("timeout -k 1s 1s playerctl next >/dev/null 2>&1"); }),
      Button("Refresh", [status] { *status = playback_status(); }),
  });
  return Renderer(controls, [status, controls] {
    if (status->empty()) *status = playback_status();
    const auto muted = Color::RGB(157, 164, 174);
    const auto surface = Color::RGB(43, 47, 53);
    auto card = vbox({text("NOW PLAYING") | bold | color(Color::RGB(190, 202, 216)),
                      text(*status) | color(Color::RGB(220, 224, 230)), controls->Render()});
    auto inset = vbox({text(" "), hbox({text(" "), card, text(" ")}), text(" ")});
    return vbox({text("AUDIO  /  MEDIA PLAYBACK") | bold | color(Color::RGB(220, 224, 230)),
                 text("Control the active MPRIS player in this desktop session.") | color(muted),
                 borderRounded(inset) | bgcolor(surface)});
  });
}
