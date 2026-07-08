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

## 安装

### 使用 Nix Flakes (推荐)

```bash
# 克隆仓库
git clone https://github.com/yigexuanmu/Miyu.git
cd Miyu

# 进入开发环境（自动安装所有依赖）
nix develop

# 构建
nix build

# 运行
./result/bin/miyu
```

### 手动编译

```bash
# 安装依赖 (Arch Linux)
sudo pacman -S pkg-config alsa-lib openssl sqlite

# 编译
cargo build --release

# 运行
./target/release/miyu
```

## 使用

编辑或写入文件时自动显示变更对比：
```bash
miyu "帮我修改 README.md"
```
