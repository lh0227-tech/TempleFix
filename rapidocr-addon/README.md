# TempleFix RapidOCR 选装组件

这个目录只包含选装组件的源码和构建说明，不会进入 TempleFix 主安装包。

组件在本机运行 RapidOCR。它通过标准输入接收截图，通过标准输出返回文字、位置和每行置信度；不会上传图片。

发布时生成独立的 TempleFix_RapidOCR_Addon_*.zip。普通用户在 TempleFix 首选项中点击“一键安装”，程序会从国内发布源下载、校验并显示安装进度；本地选择 ZIP 仅保留为高级备用入口。

建议使用 64 位 Python 3.12。在本目录运行：

    powershell -ExecutionPolicy Bypass -File build_addon.ps1 -PythonExe C:\path\to\python.exe

脚本会在 build 目录创建隔离环境、按 requirements-lock.txt 安装固定版本的依赖、构建工作进程、收集许可证并生成最终 ZIP。详细步骤是：

1. 创建独立 Python 3.12 环境并安装 requirements-lock.txt。
2. 只安装无界面的 opencv-python-headless，组件不包含桌面 GUI 依赖。
3. 使用 PyInstaller 的 onedir 模式构建 worker.py，并收集 RapidOCR 数据文件与 ONNX Runtime。
4. 运行 package_addon.ps1；脚本收集许可证、生成组件清单、校验值和最终 ZIP。

上游项目：

- RapidOCR: https://github.com/RapidAI/RapidOCR
- PaddleOCR: https://github.com/PaddlePaddle/PaddleOCR
- ONNX Runtime: https://github.com/microsoft/onnxruntime

## 国内发布源

`modelscope` 目录保存魔搭 ModelScope 发布页说明和固定的版本信息。运行 `prepare_modelscope_release.ps1` 会校验正式 ZIP，并把上传所需文件准备到 `modelscope-upload` 目录。发布后，把匿名直链写入 `src-tauri/rapidocr-release.json`，重新构建主程序即可启用“一键安装”。
