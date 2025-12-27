---
trigger: always_on
description: 编写 Rust 测试时
---

本项目中 Rust 测试需存放在单独的 `src-tauri\tests` 模块中，同时需要注意合理划分 tests 中的二级子模块，将新增测试放置在合适的二级模块中，当你有充足的理由（测试有共同语义、计划测试量足够多等）时，可以考虑新增二级测试模块。
