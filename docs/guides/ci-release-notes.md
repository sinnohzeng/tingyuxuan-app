# CI/CD Release 构建踩坑记录

本文档记录 v0.4.0 Release 工作流构建过程中遇到的问题及解决方案，供后续维护参考。

---

## 1. Tauri 2.x `target/` 目录位置

**问题**：`npx tauri build` 将构建产物输出到 **workspace root** 的 `target/release/bundle/`，而非 `src-tauri/target/release/bundle/`。

**原因**：Tauri 2.x 使用 Cargo workspace，target 目录在 workspace root。而 `cargo build --manifest-path src-tauri/Cargo.toml` 则会使用 `src-tauri/target/`。

**解决**：
- Release workflow（使用 `npx tauri build`）：路径为 `target/release/bundle/`
- CI workflow（使用 `cargo build --manifest-path`）：路径为 `src-tauri/target/`

> **规则**：修改 CI/Release workflow 时，务必确认构建命令对应的 target 目录。

---

## 2. AGP 9.0.1 需要 Gradle 9.1+

**问题**：`Minimum supported Gradle version is 9.1.0. Current version is 8.12.`

**原因**：AGP 9.0.x 大幅提升了 Gradle 最低版本要求，从 8.x 跳到 9.1.0。

**解决**：`gradle-wrapper.properties` 中设置 `distributionUrl=gradle-9.1.0-bin.zip`。

> **规则**：升级 AGP 前先查看其 [release notes](https://developer.android.com/build/releases/gradle-plugin) 中的 Gradle 兼容性矩阵。

---

## 3. AGP 9.0 内置 Kotlin 支持

**问题**：`The 'org.jetbrains.kotlin.android' plugin is no longer required for Kotlin support since AGP 9.0.`

**原因**：AGP 9.0 将 Kotlin 编译集成到了 Android Gradle Plugin 内部，不再需要单独的 `org.jetbrains.kotlin.android` 插件。

**解决**：从 `build.gradle.kts`（root 和 app 两个文件）中移除该插件声明：
```kotlin
// 移除:
// id("org.jetbrains.kotlin.android") version "2.3.10" apply false  // root
// id("org.jetbrains.kotlin.android")  // app
```

> **注意**：`org.jetbrains.kotlin.plugin.compose` 仍然需要保留。

---

## 4. AGP 9.0 移除 `kotlinOptions` DSL

**问题**：`Unresolved reference 'kotlinOptions'.`

**原因**：AGP 9.0 移除了 `android { kotlinOptions { } }` DSL 块。

**解决**：改用顶层 `kotlin { compilerOptions { } }` 块：
```kotlin
// 旧写法（AGP 8.x）:
android {
    kotlinOptions {
        jvmTarget = "21"
    }
}

// 新写法（AGP 9.0+）:
kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_21
    }
}
```

---

## 5. Material3 XML 主题资源缺失

**问题**：`AAPT: error: resource style/Theme.Material3.DayNight.NoActionBar not found.`

**原因**：`AndroidManifest.xml` 引用了 `@style/Theme.Material3.DayNight.NoActionBar`，但项目未引入 `com.google.android.material` 库（Compose 项目使用 Compose Material3，不自动提供 XML 主题资源）。

**解决**：创建自定义主题继承系统内置主题：
```xml
<!-- res/values/themes.xml -->
<style name="Theme.TingYuXuan" parent="android:Theme.Material.Light.NoActionBar" />
```

> **规则**：Compose 项目中 XML 主题仅用于 Application/Activity 声明，应使用系统自带主题或引入 Material 库。

---

## 6. Gradle GC Thrashing (OOM)

**问题**：`Gradle build daemon has been stopped: since the JVM garbage collector is thrashing`

**原因**：Gradle 9.1 + AGP 9.0.1 + R8 minification 在 GitHub Actions `ubuntu-latest`（7GB RAM）上内存不足。

**解决**：
1. 在 `gradle.properties` 中增加 JVM 堆内存：
   ```properties
   org.gradle.jvmargs=-Xmx4g -XX:+UseParallelGC
   ```
2. 暂时禁用 R8 minification（`isMinifyEnabled = false`），待配置 release 签名后再启用。

> **规则**：AGP 9.0 + R8 需要至少 4GB 堆。如果后续需要启用 R8，考虑使用 `ubuntu-latest-xl` runner 或拆分构建步骤。

---

## 7. `settings.gradle.kts` 缺少 `pluginManagement`

**问题**：`Plugin [id: 'com.android.application', version: '9.0.1'] was not found in any of the following sources`

**原因**：没有配置 `pluginManagement` 仓库，Gradle 只在 Central Plugin Repository 查找，而 AGP 发布在 Google Maven 仓库。

**解决**：
```kotlin
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode = RepositoriesMode.FAIL_ON_PROJECT_REPOS
    repositories {
        google()
        mavenCentral()
    }
}
```

---

## 8. GitHub Actions Release Workflow 竞态

**问题**：多个 build job 各自创建 draft release → 并发竞态 → 部分产物丢失。

**解决**：采用 fan-out / fan-in 架构：
```
build-linux ─────┐
build-windows ───┤──→ create-release（收集所有产物，统一创建 Release）
build-android ───┘
```

- Build jobs 使用 `actions/upload-artifact@v4`
- create-release job 使用 `actions/download-artifact@v4` + `softprops/action-gh-release@v2`

---

## 9. Tag 推送时机

**问题**：和 commits 一起推送的 tags 不一定触发 `on: push: tags` 工作流。

**解决**：先 push commits，再单独 push tags。如果仍然不触发，删除 tag 重新推送：
```bash
git push origin :refs/tags/v0.4.0
git tag -d v0.4.0
git tag -a v0.4.0 -m "..."
git push origin v0.4.0
```

---

## Quick Reference: AGP 9.0 迁移清单

- [ ] Gradle wrapper → 9.1.0+
- [ ] 移除 `org.jetbrains.kotlin.android` 插件
- [ ] `kotlinOptions {}` → `kotlin { compilerOptions {} }`
- [ ] `settings.gradle.kts` 添加 `pluginManagement` + `dependencyResolutionManagement`
- [ ] XML 主题不依赖 Material library（使用系统主题或自定义）
- [ ] `gradle.properties` 设置 `org.gradle.jvmargs=-Xmx4g`
- [ ] R8 minification 在 CI 内存受限时暂时禁用
