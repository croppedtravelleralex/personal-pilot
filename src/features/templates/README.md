# Templates Feature

负责模板元数据、变量绑定、模板存储与执行前编译。

建议内部继续拆成：

- `store.ts`
  - template list
  - selected template
  - variable bindings
- `hooks.ts`
  - save template
  - bind profiles
  - compile template request
- `model.ts`
  - template metadata
  - template variable
  - template binding

二级子模块边界：

- Template Metadata
- Variable Extraction
- Run Bindings
- Template Compiler

预期依赖的桌面契约：

- `saveTemplate`
- `updateTemplate`
- `deleteTemplate`
- `compileTemplateRun`
