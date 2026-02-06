#!/bin/bash

clean_dir() {
  [ -e "$1" ] && rm -rf "$1"
}
clean_dir web/dist
clean_dir emu/target
clean_dir rom/target
[ -e rom/rom.gb ] && rm rom/rom.gb
[ -d ios/gb-ios/RustLib ] && rm -rf ios/gb-ios/RustLib/*.a
