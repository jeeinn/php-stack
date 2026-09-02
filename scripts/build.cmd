@echo off
rem Windows 编译脚本入口
rem 用法：scripts\build.cmd [-DebugBuild] [-NoBundle] [-SkipInstall] [-Help]

powershell -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*
