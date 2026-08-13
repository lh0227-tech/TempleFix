# TempleFix 更新发布说明

应用更新以 GitHub Releases 为正式来源。Gitee 是可选的国内镜像：未配置 Gitee 时，应用仍可通过 GitHub 正常检查、下载并安装更新；以后配置镜像后，简体中文界面会优先尝试 Gitee，失败时自动回退到 GitHub。任何来源下载的更新包都必须通过同一把 Tauri 更新签名公钥校验。

## 一次性准备

1. 按 Tauri 官方方式生成更新签名密钥。私钥至少保留两份离线备份，绝不能提交到仓库。丢失私钥后，已安装的旧版本将无法再接受新版本更新。
2. 在 GitHub 仓库 Secrets 中配置 `TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。
3. 公钥保存在 `src-tauri/updater-public.key`，并与 `src-tauri/tauri.updater.conf.json` 保持一致。公钥不是秘密，仓库中的这份文件是唯一可信来源。
4. Gitee 镜像暂时不需要配置。以后启用时，将公开、长期稳定的 Gitee `latest.json` 地址配置为 GitHub 仓库变量 `TEMPLEFIX_GITEE_UPDATE_ENDPOINT`。

## 每次发布到 GitHub

1. 同步修改 `src-tauri/Cargo.toml` 与 `src-tauri/tauri.conf.json` 的版本号。
2. 在 GitHub Actions 中手动运行 `Prepare signed Windows release`。它会先执行完整检查，再生成 MSI、签名更新包、签名文件与 `latest.json`，最后只创建 GitHub 草稿发行版。
3. 检查草稿中的版本号、说明、安装包、更新包签名和 `latest.json`，确认后再公开发行版。
4. 打开 `https://github.com/lh0227-tech/TempleFix/releases/latest/download/latest.json`，确认它指向刚发布的版本。

工作流只有手动入口，普通代码推送不会自动发布任何安装包。

## 以后增加 Gitee 镜像

1. 创建公开的 Gitee 镜像仓库，并确定一个无需登录即可访问、长期不变的 `latest.json` 地址。
2. 将 GitHub 生成的同一套已签名更新文件镜像到 Gitee Release。
3. 用 `scripts/prepare_gitee_update.ps1` 把 GitHub 的 `latest.json` 转成指向 Gitee 附件的版本，再上传到约定的固定地址。
4. 验证 Gitee 与 GitHub 两个来源的元数据和附件都能匿名下载后，再设置 `TEMPLEFIX_GITEE_UPDATE_ENDPOINT` 并发布下一个版本。

示例：

```powershell
./scripts/prepare_gitee_update.ps1 `
  -LatestJsonPath .\latest.json `
  -GiteeBundleUrl "从 Gitee 页面复制的准确公开下载地址" `
  -OutputPath .\latest.gitee.json
```

## 依据

- [Tauri Updater 官方文档](https://v2.tauri.app/plugin/updater/)
- [Tauri GitHub 发布流水线官方文档](https://v2.tauri.app/distribute/pipelines/github/)
