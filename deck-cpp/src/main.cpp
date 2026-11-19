#include "tab_lighting.h"
#include "tab_audio.h"
#include "tab_lcd.h"
#include "tab_macros.h"

#include <unistd.h>
#include <iostream>

#include <string>
#include <vector>

#include <ftxui/component/component.hpp>
#include <ftxui/component/component_base.hpp>
#include <ftxui/component/event.hpp>
#include <ftxui/component/screen_interactive.hpp>
#include <ftxui/dom/elements.hpp>

int main() {
  using namespace ftxui;

  auto screen = ScreenInteractive::Fullscreen();
  const std::vector<std::string> labels = {"Lighting", "Macros", "LCD", "Audio"};
  int selected = 0;
  auto tabs = Toggle(&labels, &selected);
  if (!isatty(STDIN_FILENO) || !isatty(STDOUT_FILENO)) {
    std::cerr << "G510s Deck needs an interactive terminal.\n";
    return 0;
  }
  auto content = Container::Tab({LightingTab(screen), MacrosTab(), LcdTab(), AudioTab()},
                                &selected);
  auto view = Renderer(Container::Vertical({tabs, content}), [&] {
    const auto muted = Color::RGB(157, 164, 174);
    const auto accent = Color::RGB(190, 202, 216);
    auto body = vbox({
        hbox({text(" G510s Deck ") | bold | color(accent), text("  CONTROL CENTER") | dim,
              separator(), tabs->Render()}),
        text("LIGHTING  /  MACROS  /  LCD  /  AUDIO") | color(muted) | dim,
        separator(),
        content->Render() | flex,
        separator(),
        text("Ctrl+Left/Right: switch section    Tab: next control    Esc: quit") | color(muted),
    });
    return borderRounded(vbox({text(" "),
                               hbox({text(" "), body, text(" ")}),
                               text(" ")})) |
           color(Color::RGB(205, 210, 218)) | bgcolor(Color::RGB(35, 38, 43)) | xflex;
  });

  auto app = CatchEvent(view, [&](Event event) {
    // Let a focused text field consume 'q' before treating it as quit.
    if (event == Event::q && view->OnEvent(event)) return true;
    if (event == Event::Escape || event == Event::q) {
      screen.ExitLoopClosure()();
      return true;
    }
    if (event == Event::ArrowRightCtrl || event == Event::ArrowLeftCtrl) {
      const int count = static_cast<int>(labels.size());
      const int step = event == Event::ArrowRightCtrl ? 1 : -1;
      selected = (selected + step + count) % count;
      content->TakeFocus();
      return true;
    }
    return false;
  });
  // Save and restore the terminal title around the fullscreen session.
  std::cout << "\033[22;2t\033]2;G510s Deck\007" << std::flush;
  screen.Loop(app);
  std::cout << "\033[23;2t" << std::flush;
}
