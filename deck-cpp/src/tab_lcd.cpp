#include "tab_lcd.h"
#include "ipc.h"

#include <ftxui/component/component.hpp>
#include <ftxui/component/component_options.hpp>
#include <ftxui/dom/elements.hpp>

#include <exception>
#include <memory>
#include <string>
#include <vector>

ftxui::Component LcdTab() {
  using namespace ftxui;
  struct State {
    std::string line1, line2, status, mirror = "No LCD snapshot requested.";
    int duration = 1;
    std::vector<std::string> labels{"1 s", "2.5 s", "5 s", "10 s"};
    IpcClient ipc;
  };
  auto state = std::make_shared<State>();
  InputOption option;
  option.multiline = false;
  auto first = Input(&state->line1, "First line", option);
  auto second = Input(&state->line2, "Second line", option);
  auto duration = Toggle(&state->labels, &state->duration);
  auto send = Button("Send", [state] {
    if (state->line1.empty() && state->line2.empty()) {
      state->status = "Enter a toast message first.";
      return;
    }
    constexpr unsigned milliseconds[] = {1000, 2500, 5000, 10000};
    try {
      auto reply = state->ipc.toast(state->line1, state->line2, milliseconds[state->duration]);
      state->status = reply.at("ok").get<bool>() ? "Toast sent."
        : "Send failed: " + reply.value("error", std::string("request rejected"));
    } catch (const std::exception& e) {
      state->status = std::string("Send failed: ") + e.what();
    }
  });
  auto mirror = Button("Refresh LCD mirror", [state] {
    try {
      const auto reply = state->ipc.request("lcd_mirror");
      if (!reply.at("ok").get<bool>()) {
        state->mirror = "Mirror unavailable: " + reply.value("error", std::string("request rejected"));
        return;
      }
      const auto& data = reply.at("data");
      const auto& bytes = data.at("data");
      if (data.at("encoding") != "g510_report3_bytes" || bytes.size() != 992) {
        state->mirror = "Invalid LCD mirror frame.";
        return;
      }
      std::string preview;
      for (int y = 0; y < 43; y += 2) {
        for (int x = 0; x < 160; x += 2) {
          bool on = false;
          for (int dy = 0; dy < 2 && y + dy < 43; ++dy) {
            const int bit = (y + dy) % 8;
            const int offset = 32 + ((y + dy) / 8) * 160 + x;
            on |= (bytes.at(offset).get<unsigned>() & (1u << bit)) != 0;
            if (x + 1 < 160) on |= (bytes.at(offset + 1).get<unsigned>() & (1u << bit)) != 0;
          }
          preview += on ? "##" : "  ";
        }
        preview += '\n';
      }
      state->mirror = std::move(preview);
    } catch (const std::exception& e) {
      state->mirror = std::string("Mirror failed: ") + e.what();
    }
  });
  auto controls = Container::Vertical({first, second, duration, send, mirror});
  return Renderer(controls, [=] {
    const auto muted = Color::RGB(157, 164, 174);
    const auto surface = Color::RGB(43, 47, 53);
    auto panel = [&](Elements body) {
      auto content = vbox(std::move(body));
      return borderRounded(vbox({text(" "), hbox({text(" "), content, text(" ")}), text(" ")})) |
             bgcolor(surface);
    };
    return vbox({
          text("LCD  /  ON-DEVICE MESSAGES") | bold | color(Color::RGB(220, 224, 230)),
          text("Send a two-line toast or inspect the current display.") | color(muted),
          panel({text("COMPOSE TOAST") | bold | color(Color::RGB(190, 202, 216)),
                 hbox({text("Line 1: "), first->Render() | flex}),
                 hbox({text("Line 2: "), second->Render() | flex}),
                 hbox({text("Duration: "), duration->Render()}), send->Render(),
                 paragraph(state->status) | color(muted)}),
          panel({text("LCD MIRROR  ·  160 × 43  ·  2× PREVIEW") | bold | color(Color::RGB(190, 202, 216)),
                 mirror->Render(), paragraph(state->mirror) | borderRounded}),
    }) | yframe | vscroll_indicator;
  });
}
