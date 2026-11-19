#include "tab_macros.h"

#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <memory>
#include <sstream>
#include <string>
#include <vector>

#include <ftxui/component/component.hpp>
#include <ftxui/component/event.hpp>
#include <ftxui/dom/elements.hpp>
#include <nlohmann/json.hpp>

namespace {
using Json = nlohmann::json;

std::string Summary(const Json& action) {
  const auto type = action.find("type");
  if (type != action.end()) {
    if (*type == "recorded") {
      auto events = action.find("events");
      if (events == action.end() || events->is_array()) {
        return "recorded: " + std::to_string(events == action.end() ? 0 : events->size()) + " events";
      }
    } else if (*type == "exec" || *type == "keys") {
      auto args = action.find(*type == "exec" ? "argv" : "keys");
      if (args != action.end() && args->is_array()) {
        std::string label = type->get<std::string>() + ":";
        for (const auto& arg : *args) {
          if (!arg.is_string()) return "Invalid binding";
          label += " " + arg.dump(-1, ' ', true);
          if (label.size() > 72) return label.substr(0, 69) + "...";
        }
        return label;
      }
    }
  }
  return "Invalid binding";
}

struct Browser {
  int slot = 0, line = 0, page = 0;
  std::vector<std::string> labels, details, lines;
  std::string status;

  Browser() {
    const auto* home = std::getenv("HOME");
    auto path = std::filesystem::path(home ? home : "") / ".config/g510s-drive/config.json";
    std::ifstream input(path);
    Json cfg;
    if (!input) status = "Cannot read " + path.string();
    else {
      try {
        cfg = Json::parse(input);
        if (!cfg.contains("profiles")) status = "No explicit profiles in config";
        else if (!cfg.at("profiles").is_object()) status = "Invalid profiles in config";
      } catch (const Json::exception&) {
        status = "Invalid JSON in " + path.string();
      }
    }
    for (int m = 1; m <= 3; ++m) {
      auto profile = "M" + std::to_string(m);
      for (int g = 1; g <= 18; ++g) {
        auto key = "G" + std::to_string(g);
        std::string summary = status.empty() ? "Unbound" : "Unavailable";
        std::string detail = summary;
        if (status.empty() && cfg.at("profiles").contains(profile)) {
          const auto& bindings = cfg.at("profiles").at(profile);
          if (!bindings.is_object()) summary = detail = "Invalid profile";
          else if (bindings.contains(key)) {
            const auto& action = bindings.at(key);
            summary = Summary(action);
            detail = action.dump(2, ' ', true);
          }
        }
        labels.push_back(profile + " " + key + "  " + summary);
        details.push_back(detail);
      }
    }
  }
};
}  // namespace

ftxui::Component MacrosTab() {
  using namespace ftxui;
  auto state = std::make_shared<Browser>();
  auto slots = Menu(&state->labels, &state->slot);
  auto lines = Menu(&state->lines, &state->line);
  auto pages = Container::Tab({slots, lines}, &state->page);
  auto view = Renderer(pages, [state, pages] {
    auto title = state->page ? state->labels[state->slot] : "Macros (54 slots, read-only)";
    const auto muted = Color::RGB(157, 164, 174);
    const auto surface = Color::RGB(43, 47, 53);
    auto body = vbox({text(title) | bold | color(Color::RGB(190, 202, 216)),
                      text(state->status.empty() ? (state->page ? "Enter: back to bindings" : "Enter: inspect binding details") : state->status) | color(muted),
                      pages->Render() | vscroll_indicator | yframe | flex});
    auto inset = vbox({text(" "), hbox({text(" "), body, text(" ")}), text(" ")});
    return vbox({text("MACROS  /  KEY BINDINGS") | bold | color(Color::RGB(220, 224, 230)),
                 borderRounded(inset) | bgcolor(surface)});
  });
  return CatchEvent(view, [state](Event e) {
    if (e != Event::Return) return false;
    if (!state->page) {
      state->lines.clear();
      std::istringstream raw(state->details[state->slot]);
      for (std::string s; std::getline(raw, s);) state->lines.push_back(s);
      state->line = 0;
    }
    state->page = 1 - state->page;
    return true;
  });
}
