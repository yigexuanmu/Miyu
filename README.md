# Miyu - Nix打包

基于 Rust 的命令行 AI 助手


## NixOS 安装

### 1. 在 flake.nix 中添加输入

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    miyu.url = "github:yigexuanmu/Miyu";
  };
}
```

### 2. 在 configuration.nix 中添加

```nix
{ inputs, ... }:

{
  environment.systemPackages = [ inputs.miyu.packages.x86_64-linux.default ];
}
```

### Home Manager

```nix
{ inputs, ... }:

{
  home.packages = [ inputs.miyu.packages.x86_64-linux.default ];
}
```

## 致谢

- [SHORiN-KiWATA/Miyu](https://github.com/SHORiN-KiWATA/Miyu) - 原项目
