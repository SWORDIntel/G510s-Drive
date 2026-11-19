cmake_minimum_required(VERSION 3.20)
project(g510deck LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

include(FetchContent)
FetchContent_Declare(ftxui
  GIT_REPOSITORY https://github.com/ArthurSonzogni/FTXUI.git
  GIT_TAG v6.1.1
  GIT_SHALLOW TRUE)
FetchContent_Declare(nlohmann_json
  GIT_REPOSITORY https://github.com/nlohmann/json.git
  GIT_TAG v3.11.3
  GIT_SHALLOW TRUE)
FetchContent_MakeAvailable(ftxui nlohmann_json)

# CMake needs one translation unit until deck_core has its first source.
file(GENERATE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/deck_core.cpp" CONTENT "// Intentionally empty.\n")
add_library(deck_core STATIC "${CMAKE_CURRENT_BINARY_DIR}/deck_core.cpp")
target_sources(deck_core PRIVATE src/ipc.cpp)
target_link_libraries(deck_core PUBLIC nlohmann_json::nlohmann_json)

add_executable(ipc_selftest src/ipc_selftest.cpp)
target_link_libraries(ipc_selftest PRIVATE deck_core)

add_executable(g510deck-cpp src/main.cpp)
target_sources(g510deck-cpp PRIVATE
  src/tab_lighting.cpp src/tab_macros.cpp src/tab_lcd.cpp src/tab_audio.cpp)
target_compile_options(g510deck-cpp PRIVATE -Wall -Wextra)
target_link_libraries(g510deck-cpp PRIVATE deck_core ftxui::component)
