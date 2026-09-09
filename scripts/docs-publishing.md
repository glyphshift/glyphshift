# 文档维护与发布

## 文件放哪里

用户文档放在根目录 `docs/`，入口是 `docs/index.md`。一个主题一个 Markdown，使用 UTF-8。

文件名就是导入路径，发过的页面尽量不要随意改名或移动，否则需要在文档站处理旧页面。

`docs/docs.json` 声明默认语言。目前先维护简体中文；以后加英文时，可以放到 `docs/locales/en-US/` 并在清单里增加对应语言。相同页面的 front matter `id` 保持一致。

内部实现笔记继续放 Flightdeck，不打进用户文档包。当前包不带截图；以后增加图片时，先按仓库隐私规则检查，再决定打包方式。

## 本机打包

需要 Python 3.10 或以上，不用安装额外 Python 包。从仓库根目录运行：

```sh
python scripts/yueli_docs_publish.py pack docs --output local-test/evidence/docs-release/docs.zip
```

ZIP 根目录直接是页面和 `docs.json`，不会多套一层 `docs/`。脚本只收录 Markdown、清单和支持的图片，输出位置不参与版本管理。

`scripts/yueli_docs_publish.py` 来自内置的 yueli-docs-publish 技能，已复制进仓库。CI 不依赖开发者本机安装的技能。

## Release 自动打包

现有 `.github/workflows/release.yml` 在 `vMAJOR.MINOR.PATCH` 标签推送后运行。版本校验通过后打包文档，随后与安装包一起创建 Release。

发布附件包含：

- Windows 安装包
- `candidate-manifest.json`
- `docs.zip`
- `SHA256SUMS.txt`，包含安装包和文档包的校验值

文档打包失败会阻止发布。附件来自同一个标签的 checkout，不在发布后另外抓取主分支文档。

文档打包直接接入现有 job，因为仓库令牌创建的 Release 不会再触发普通的 release 事件工作流。见 [GitHub 的令牌触发规则](https://docs.github.com/en/actions/concepts/security/github_token)。

## 后续接月离文档

在月离文档的“批量导入 → 项目同步”里，绑定公开仓库、Release 附件 `docs.zip` 和目标文档集。文档集决定发布到哪里，包里不写死目标。

后续维护 `docs/`，随软件发版更新文档包即可。不需要为了打包文档给 GitHub 配文档站令牌。

也可以手动预检：

```sh
python scripts/yueli_docs_publish.py preflight local-test/evidence/docs-release/docs.zip --base https://docs.yuelili.com --collection <collection-slug> --locale zh-CN
```

预检或发布需要在环境变量 `YUELI_DOCS_TOKEN` 中配置有文档导入权限的开发者令牌。不要把令牌写进仓库或命令参数。

预检会上传包，但不会确认发布。确定要发布时，使用 `publish` 代替 `preflight`；默认按语言和路径更新，包里没出现的旧页面保留。确认请求超时后先用 `status <batch-id>` 查询，不要直接重复提交。

这里只准备了文件和 Release 打包步骤。实际推送标签、发布 Release 和绑定文档站是后续操作。
