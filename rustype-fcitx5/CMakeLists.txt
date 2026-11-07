cmake_minimum_required(VERSION 3.16)
project(rustype-fcitx5 VERSION 0.1.0 LANGUAGES C CXX)

find_package(Fcitx5Core 5.0.0 REQUIRED)
find_package(Fcitx5Module 5.0.0 REQUIRED)

set(RUSTYPE_CORE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../rustype-core")
include_directories("${RUSTYPE_CORE_DIR}/include")

add_library(rustype MODULE
    src/rustype_engine.cpp
)

target_link_libraries(rustype
    PRIVATE
    Fcitx5::Core
    "${CMAKE_CURRENT_SOURCE_DIR}/../target/release/librustype_core.a"
    pthread
    dl
    m
)

set_target_properties(rustype PROPERTIES PREFIX "")
install(TARGETS rustype DESTINATION "${FCITX_INSTALL_PKGLIBDIR}")
install(FILES rustype.conf DESTINATION "${FCITX_INSTALL_PKGDATADIR}/inputmethod")
install(FILES rustype-addon.conf DESTINATION "${FCITX_INSTALL_PKGDATADIR}/addon")
