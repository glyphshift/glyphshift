# 历史词典源

本目录保存尚未完成产品化导入的历史词典数据，防止仓库迁移时丢失有价值的翻译内容。

- 文件保持原 Dictionary schema 2 内容，仅用于一次性导入工具的输入。
- Desktop Backend、Runtime、Extension Registry 和 GUI 不得直接读取本目录。
- 导入必须显式选择目标 Dictionary、Location、Context 与 Hook scope，并报告无法无歧义映射的
  Domain；不能把旧 Domain 路径偷偷塞回新的 Replacement Rule。
- 导入完成且生成的新词典经过规则数、冲突和实机结果核对后，才可以删除对应源文件。

After Effects 文件是当前唯一完整的大型历史词典。Premiere Pro 文件包含迁移时工作树中的最新
用户内容，必须在任何清理前保留。
