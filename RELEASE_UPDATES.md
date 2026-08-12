# TempleFix 更新发布说明

应用内更新采用“GitHub 正式源 + Gitee 国内镜像”的双源方案：简体中文界面先检查 Gitee，其他界面先检查 GitHub；任一来源失败时继续尝试另一来源。安装包无论从哪里下载，都必须通过同一把 Tauri 更新签名公钥校验。

## 一次性准备

1. 按 Tauri 官方命令生成更新签名密钥。私钥至少保留两份离线备份，绝不能提交到仓库；丢失私钥后，已经安装的旧版本将无法再接受新版本更新。
2. 在 GitHub 仓库 Secrets 中配置 `TAURI_SIGNING_PRIVATE_KEY`、可选的 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`，以及 `TEMPLEFIX_UPDATER_PUBKEY`。
3. 创建公开的 Gitee 镜像仓库，确定一个长期不变、可匿名访问的 `latest.json` 地址，并把它配置为 GitHub 仓库变量 `TEMPLEFIX_GITEE_UPDATE_ENDPOINT`。

仓库不会猜测 Gitee 用户名、仓库名或附件地址。缺少以上配置时，发布流程会主动失败；普通本地构建仍可运行，但不会联网检查更新。

## 每次发布

1. 同步修改 `src-tauri/Cargo.toml` 与 `src-tauri/tauri.conf.json` 的版本号。
2. 在 GitHub Actions 中手动运行 `Prepare signed Windows release`。它先执行完整检查，再生成 MSI、签名更新包、签名文件与 `latest.json`，最后只创建 GitHub 草稿发行版。
3. 将同一个已签名更新包镜像到 Gitee Release，并复制 Gitee 给出的准确、公开、无需登录的下载地址。
4. 用 `scripts/prepare_gitee_update.ps1` 把 GitHub 生成的 `latest.json` 转成指向 Gitee 附件的版本，再发布到步骤 3 中约定的固定 Gitee 元数据地址。脚本只生成文件，不会上传。
5. 分别验证 Gitee 和 GitHub 的 `latest.json` 与安装包下载；确认后再公开 GitHub 草稿发行版。

示例：

```powershell
./scripts/prepare_gitee_update.ps1 `
  -LatestJsonPath .\latest.json `
  -GiteeBundleUrl "从 Gitee 页面复制的准确公开下载地址" `
  -OutputPath .\latest.gitee.json
```

发布工作流只有手动入口，并默认生成草稿；仅把工作流文件放进仓库不会自动上传任何内容。

## 依据

- [Tauri Updater 官方文档](https://v2.tauri.app/plugin/updater/)
- [Tauri GitHub 发布流水线官方文档](https://v2.tauri.app/distribute/pipelines/github/)
- [Gitee OpenAPI Release 附件接口](https://gitee.com/sdk/gitee5j/blob/main/docs/RepositoriesApi.md)
