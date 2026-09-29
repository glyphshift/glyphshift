# 文档维护与发布

公开使用指南的唯一正文来源为 [glyphshift/docs](https://github.com/glyphshift/docs) 的 `content/`，阅读入口为 https://glyphshift.yuelili.com/docs/。主仓 `docs/` 保留旧文件路径的迁移导航，以及与实现一同维护的架构、SDK、插件和 Registry 合同；不再维护第二份使用指南。

## 固定文档版本

`docs-source.lock.json` 固定 Docs 仓库的完整 commit。更新使用指南后，先让 Docs 构建与内容检查通过，再在主仓更新此锁。App 发版只消费锁定提交，不能在发布时读取 Docs 的浮动 main。

Release Action 读取并验证锁、checkout 独立公开 Docs 提交到忽略目录，再调用已有文档打包器。`docs.zip` 与安装包一起上传并写入 SHA256SUMS；正文位于包内 `zh/`，`docs.json` 保留 zh-CN 语言和各页稳定 id。文档拉取或打包失败会阻止发布。此次来源切换不会自行创建软件 Release。

## 本地打包

在独立 Docs checkout 运行：

```text
python scripts/yueli_docs_publish.py pack content --output local-test/docs.zip
```

若要复验主仓 Release 的精确输入，检出锁定的 Docs commit，使用主仓 `scripts/yueli_docs_publish.py` 对该 checkout 的 `content/` 打包。不要把相邻仓浮动工作区当作正式依赖。

## 月离文档同步

已有月离文档 URL 暂保留，不删除线上内容。若继续配置 Release docs.zip 同步，应让其消费同一个版本化包，而不是重新维护正文。此轮没有修改月离文档站绑定或发布软件 Release。
