@echo off
set http_proxy=http://127.0.0.1:7897
set https_proxy=http://127.0.0.1:7897

:: 切换到 bat 自己所在的目录
cd /d "%~dp0"

echo 当前目录：%cd%
echo 代理已设置：%http_proxy%

grok
pause