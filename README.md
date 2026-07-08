# Miyu - TUI Diff 显示

基于 Rust 的命令行 AI 助手，新增 TUI 风格的文件变更对比功能。

## 功能

- **TUI Diff 显示** - 文件编辑时显示美观的变更对比
- **彩色输出** - 红色删除、绿色添加
- **上下文控制** - 可配置显示的上下文行数

## 效果示例

```
╭─ /path/to/file.txt ────────────────────
│    1 - old content
│    1 + new content
╰────────────────────────────────────────╯
```

## 配置

```jsonc
{
  "plugins": {
    "diff_display": {
      "enabled": true,           // 启用/禁用
      "context_lines": 1,        // 上下文行数
      "show_file_header": true,  // 显示文件头
      "max_lines": 50            // 最大显示行数
    }
  }
}
```

## NixOS 安装

### 方式一：systemPackages (全局安装)

在 `configuration.nix` 中添加：

```nix
{ config, pkgs, ... }:

let
  miyu = pkgs.callPackage ./miyu.nix { };
in
{
  environment.systemPackages = [ miyu ];
}
```

创建 `miyu.nix`：

```nix
{ rustPlatform, pkg-config, alsa-lib, openssl, sqlite }:

rustPlatform.buildRustPackage {
  pname = "miyu";
  version = "0.1.10";
  src = ./.;
  cargoLock.lockFile = ./Cargo.lock;
  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ alsa-lib openssl sqlite ];
  doCheck = false;
}
```

### 方式二：Home Manager (用户级安装)

在 `home.nix` 中添加：

```nix
{ config, pkgs, ... }:

let
  miyu = pkgs.callPackage ./miyu.nix { };
in
{
  home.packages = [ miyu ];
}
```

### 方式三：nix profile

```bash
# 克隆并构建
git clone https://github.com/yigexuanmu/Miyu.git
cd Miyu
nix build

# 添加到 profile
nix profile install ./result
```

### 方式四：临时使用

```bash
git clone https://github.com/yigexuanmu/Miyu.git
cd Miyu
nix run
```

## 手动编译

```bash
# 安装依赖 (Arch Linux)
sudo pacman -S pkg-config alsa-lib openssl sqlite

# 编译
cargo build --release

# 运行
./target/release/miyu
```

## 致谢

- [SHORiN-KiWATA/Miyu](https://github.com/SHORiN-KiWATA/Miyu) - 原项目
