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

## 10. cpal 0.17 SampleRate 类型变更

**问题**：Windows CI clippy 报错 `u32 is a primitive type and therefore doesn't have fields`，Linux 本地编译通过。

**原因**：cpal 0.17 将 `SampleRate` 从 `pub struct SampleRate(pub u32)` 改为 `pub type SampleRate = u32`。代码中 `max_sample_rate().0` 在旧版本用于提取内部 `u32` 值，在新版本中 `.0` 对 `u32` 无效。

**解决**：移除 `.0` 字段访问，直接使用 `max_sample_rate()` 返回的 `u32` 值。

> **规则**：升级 cpal 版本后注意 `SampleRate` 类型变更。Linux 本地编译不能代替 Windows CI 检查（`#[cfg]` 条件编译可能隐藏平台差异）。

---

## 11. tokio-tungstenite native-tls 导致 Android 交叉编译失败

**问题**：Android Release 构建报错 `Could not find directory of OpenSSL installation`。

**原因**：`tokio-tungstenite` 使用 `native-tls` feature，底层依赖 `openssl-sys`。Android NDK 交叉编译环境没有预装 OpenSSL。而 `reqwest 0.13` 默认使用 rustls，不依赖 OpenSSL。

**解决**：将 `tokio-tungstenite` 从 `features = ["native-tls"]` 改为 `features = ["rustls-tls-webpki-roots"]`。rustls 是纯 Rust 实现，无需系统 OpenSSL。

> **规则**：涉及 TLS 的依赖在 Android 交叉编译时，优先使用 rustls 系列 feature。`reqwest 0.13+` 默认已使用 rustls。

---

## 12. Cargo.lock 与安全审计

**问题**：CI `cargo audit --file src-tauri/Cargo.lock` 报错文件不存在。

**原因**：(1) `.gitignore` 中包含 `Cargo.lock`，导致 lock 文件未提交。(2) 项目使用 Cargo workspace，`Cargo.lock` 位于 workspace 根目录，不在 `src-tauri/`。

**解决**：
1. 从 `.gitignore` 移除 `Cargo.lock`（应用项目应该提交 lock 文件）
2. CI 改为 `cargo audit --file Cargo.lock`（指向根目录的 lock 文件）

> **规则**：Rust 应用（非库）应该提交 `Cargo.lock`。Workspace 项目的 `Cargo.lock` 在 workspace 根目录。

---

## 13. Windows clippy collapsible_if

**问题**：Windows CI clippy 报错 `this if statement can be collapsed`，但 Linux 本地 clippy 不报错。

**原因**：`windows.rs` 中的嵌套 `if let` 语句只在 Windows 平台编译（`#[cfg(windows)]`），本地 Linux clippy 不检查该文件。

**解决**：将嵌套 `if let` 合并为 `if let ... && let ...` 语法（Rust 2024 edition 支持 let chains）。

> **规则**：`#[cfg(windows)]` 平台专属代码需要通过 CI 的 Windows 构建来验证 clippy 合规性。

---

## 14. R8 Tink 加密库注解缺失

**问题**：Android Release R8 minification 报错 `Missing class com.google.errorprone.annotations.*`。

**原因**：`EncryptedSharedPreferences` 依赖 Google Tink 加密库，Tink 引用了 `errorprone` 和 `javax.annotation` 编译时注解。R8 默认要求所有引用的类必须存在。

**解决**：在 `proguard-rules.pro` 中添加：
```
-dontwarn com.google.errorprone.annotations.**
-dontwarn javax.annotation.**
-dontwarn javax.annotation.concurrent.**
```

> **规则**：使用 `EncryptedSharedPreferences` 或 Tink 库时，需要配套的 ProGuard/R8 dontwarn 规则。

---

## 15. macOS 构建：CGEventTap 需要 Input Monitoring 权限

**问题**：macOS 上 Fn 键监听使用 `CGEventTap`，需要 **Input Monitoring** 权限（非辅助功能权限）。

**原因**：macOS 10.15+ 将 Input Monitoring 从 Accessibility 中独立出来。`CGEventTap` 监听键盘事件需要 Input Monitoring，而 `CGEventPost` 模拟按键需要 Accessibility。

**解决**：
- Entitlements.plist 禁用沙箱（`com.apple.security.app-sandbox = false`）
- 首次启动引导用户授予两个独立权限
- `check_permissions()` 通过 `AXIsProcessTrusted()` 检测辅助功能权限

> **规则**：macOS 文本注入类应用需要 Accessibility + Input Monitoring 两个独立权限，不能只请求其一。

---

## 16. macOS 构建：非沙箱应用必须禁用 App Sandbox

**问题**：Tauri 应用需要调用 System Events（AppleScript）和 CGEvent，App Sandbox 会阻止这些操作。

**原因**：沙箱限制了 AppleScript 自动化、全局事件监听、剪贴板操作等功能。Typeless 等同类应用均禁用沙箱。

**解决**：`Entitlements.plist` 设置 `com.apple.security.app-sandbox = false`，同时启用：
- `com.apple.security.automation.apple-events = true`
- `com.apple.security.device.audio-input = true`

> **规则**：语音输入 + 全局注入类应用无法在沙箱内运行，需禁用 App Sandbox。

---

## 17. macOS Release：DMG 构建与代码签名

**问题**：未签名 DMG 在 macOS 上会被 Gatekeeper 拦截。

**临时方案**：暂无 Apple Developer 账号，先出未签名版。用户右键 → "打开" 可绕过 Gatekeeper。

**后续方案**：购买 Apple Developer 账号后，在 Release workflow 中启用签名 + 公证：
```yaml
env:
  APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
  APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
  APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
  APPLE_ID: ${{ secrets.APPLE_ID }}
  APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
  APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
```

> **规则**：macOS 分发必须签名 + 公证才能免 Gatekeeper 弹窗。Release workflow 中已预留签名步骤注释。

---

## 18. macOS CI：无需安装系统依赖

**问题**：Linux CI 需要 `apt-get install libasound2-dev libwebkit2gtk-4.1-dev` 等，macOS 是否类似？

**答案**：macOS 自带 WebKit、CoreAudio、CoreGraphics 等框架，无需额外安装系统依赖。CI 只需 Rust toolchain + Node.js。

> **规则**：macOS CI job 比 Linux 简单 — 无 `apt-get` 步骤，直接 clippy/test/build。

---

## Quick Reference: AGP 9.0 迁移清单

- [ ] Gradle wrapper → 9.1.0+
- [ ] 移除 `org.jetbrains.kotlin.android` 插件
- [ ] `kotlinOptions {}` → `kotlin { compilerOptions {} }`
- [ ] `settings.gradle.kts` 添加 `pluginManagement` + `dependencyResolutionManagement`
- [ ] XML 主题不依赖 Material library（使用系统主题或自定义）
- [ ] `gradle.properties` 设置 `org.gradle.jvmargs=-Xmx4g`
- [ ] R8 minification 在 CI 内存受限时暂时禁用
