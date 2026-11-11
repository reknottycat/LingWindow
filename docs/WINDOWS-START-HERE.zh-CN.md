# 聆窗 LingWindow Windows 版

这是基于 AudioScript 0.7.0 的独立定制版，保留原项目的系统音频采集、流式俄语识别、翻译队列和双语悬浮字幕。它不会覆盖原版的配置，也不会通过原版自动更新替换自身。

## 安装与首次初始化

1. 安装 Windows x64 安装包（.exe 或 .msi，二选一）。这些是未签名的个人构建，安装前请核对随包 SHA256。不要关闭系统安全防护来安装来源不明的副本。
2. 安装官方 Python 3.12 x64，以及支持 CUDA 12.8 的 NVIDIA 驱动。RTX 4070 笔记本可以使用该 CUDA 后端，但实际显存余量与端到端速度仍需在你的机器上验证。预留至少 12 GB 磁盘空间。
3. 在随附 scripts 文件夹双击 setup-windows.cmd。它在用户目录建立独立 Python 环境，不改全局 Python；从国内 ModelScope 下载官方 HY-MT2 1.8B BF16 模型（约 4.1 GB），逐文件校验 SHA256 和大小。PyTorch 从其官方 CUDA wheel 源安装。
4. 打开应用的“模型”页，下载 Nemotron 3.5 识别模型。此模型使用国内 ModelScope 的同版 sherpa-onnx 镜像及原文件大小、SHA256 校验，不能把内置中英 Zipformer 当成俄语识别模型。Nemotron 许可为 OpenMDW-1.1，HY-MT2 为 Apache-2.0，原应用为 MIT。
5. 新安装默认：系统声音、俄语→中文、Nemotron 3.5、HY-MT2。点击开始转写并显示悬浮窗。字幕应同时保留俄语原文和中文翻译。

模型位于 %LOCALAPPDATA%\com.lingwindow.desktop\models\mt-hymt2。初始化后，语音识别和翻译均在本机运行，不需要云端 API。禁用所有遥测和崩溃上报。

## 先做无声测试

“模型”页可输入俄语文字，点击“测试翻译与悬浮字幕”。这会经过真实翻译队列和浮窗，不播放声音、不录音、不重跑识别。首次加载比热态慢。无声测试成功仍不代表系统音频或 GPU 端到端验收已完成。

CUDA 不可用时会明确报错，不会静默切换模型。可用显式 --cpu 参数初始化 CPU 模式，但不保证实时性。用户允许 CPU 后，其设备与警告会进入日志。

## 实时字幕验收

- 先用浏览器播放一段清晰俄语，确认原文持续更新，中文跟随最终分句输出。
- 重复开始/停止五次，暂停播放、切换输出设备，然后恢复，确认不会不断积压旧字幕。
- 试验最小化主窗、置顶、拖动浮窗、调字号、托盘退出。退出后不应留下 Python 推理进程。
- 浏览器全屏和独占全屏游戏不等价，后者未保证可以覆盖字幕。
- 观察任务管理器显存、应用日志中的实际 CUDA 设备、初始化错误和翻译延迟。ASR 当前沿用上游 CPU 后端，RTX 4070 用于 HY-MT2 翻译。

Windows CI 可以验证真正的 MSVC 编译、自动化测试和安装包生成。云端 CPU 翻译测试与模拟协议测试都不能替代 Windows 真机的音频、GPU、热键与置顶体验验收。以同批验证报告列出的已执行项目为准。

## 从旧 Windows 定制版迁移（可选，必须显式执行）

默认安装不会读取、移动或删除旧 `com.audioscript.hymt2` 目录。可以重新下载到新目录，或在 scripts 目录的 PowerShell 中显式复制已校验的官方模型：

```powershell
py -3.12 -X utf8 .\setup-windows-hymt2.py --model-dir "$env:LOCALAPPDATA\com.lingwindow.desktop\models\mt-hymt2" --copy-model-from "$env:LOCALAPPDATA\com.audioscript.hymt2\models\mt-hymt2"
```

此操作只复制清单内的 HY-MT2 官方模型文件，逐文件校验；旧目录保持不变。不会复制旧 Python 环境、runtime.json、偏好、录音或字幕记录。新目录会建立自己的运行环境，原程序仍可使用原目录。Mac 的 MLX 4-bit 权重不能用于 Windows BF16 后端。

## 署名与许可

感谢 Nico 与 AudioScript 贡献者。原项目 MIT 许可及第三方署名随安装包保留；基础 OPUS 中英模型的许可证、来源与转换说明见包内 third-party/THIRD_PARTY_NOTICES.md。可选模型各自遵循其许可证，不能把应用的 MIT 许可当作所有模型的许可。
