* 鸣潮官方启动器无法在 wine 或 proton 上正常使用，请使用社区工具（如 [wutheringwaves-cli-manager](https://github.com/timetetng/wutheringwaves-cli-manager) 或 [LutheringLaves](https://github.com/last-live/LutheringLaves)）下载和更新游戏。

* 通常情况下，国服游玩需要添加 `SteamOS=1` 环境变量。部分用户反馈需要 `steamdeck=1`。

* 如果上线 10 分钟就被踢下线，需要重新登录。可以尝试 B 站用户 `@神麤詭末` 的解决方案：修改文件 `游戏安装目录/Client/Binaries/Win64/ThirdParty/KrPcSdk_Mainland/KRSDKRes/KRSDKConfig.json`，将 `KR_ChannelId` 从 `19` 为 `205`。之后启动游戏可能会提示网络错误，点击 `重试` 即可正常进入游戏。

* 使用 proton-ge 游玩可能会偶尔闪退，可以尝试自己用 spritz-wine 和 vkd3d-proton 搭建运行环境。
