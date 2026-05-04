@echo off
set PROBE_RS=..\probe-rs\target\release\probe-rs
set ELF=%1

echo -- Flashing --
"%PROBE_RS%" download --chip CH32H417 --chip-erase "%ELF%"
echo -- Attaching RTT --
"%PROBE_RS%" attach --chip CH32H417 --no-catch-reset --no-catch-hardfault "%ELF%"
