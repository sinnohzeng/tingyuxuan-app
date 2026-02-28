package com.tingyuxuan.ime.model

import org.json.JSONObject

/**
 * 输入上下文数据 — 对应 Rust 侧 `tingyuxuan_core::context::InputContext`。
 *
 * 字段名与 Rust serde snake_case 完全一致，通过 [toJson] 序列化为 JSON
 * 传递给 JNI 层。
 *
 * Android IME 天然可获取丰富上下文：
 * - EditorInfo: packageName, inputType, hintText, imeOptions
 * - InputConnection: getTextBeforeCursor, getSelectedText
 * - ClipboardManager: primaryClip
 */
data class InputContextData(
    // 应用信息
    val appName: String? = null,
    val appPackage: String? = null,
    val windowTitle: String? = null,

    // 浏览器信息
    val browserUrl: String? = null,

    // 输入框信息（Android 通过 EditorInfo 获取）
    val inputFieldType: String? = null,
    val inputHint: String? = null,
    val editorAction: String? = null,

    // 文本上下文
    val surroundingText: String? = null,
    val selectedText: String? = null,
    val clipboardText: String? = null,
    val screenText: String? = null,
) {
    /**
     * 序列化为 JSON 字符串，字段名使用 snake_case 与 Rust serde 一致。
     * 仅包含非空字段，减少传输体积。
     */
    fun toJson(): String {
        val obj = JSONObject()
        appName?.let { obj.put("app_name", it) }
        appPackage?.let { obj.put("app_package", it) }
        windowTitle?.let { obj.put("window_title", it) }
        browserUrl?.let { obj.put("browser_url", it) }
        inputFieldType?.let { obj.put("input_field_type", it) }
        inputHint?.let { obj.put("input_hint", it) }
        editorAction?.let { obj.put("editor_action", it) }
        surroundingText?.let { obj.put("surrounding_text", it) }
        selectedText?.let { obj.put("selected_text", it) }
        clipboardText?.let { obj.put("clipboard_text", it) }
        screenText?.let { obj.put("screen_text", it) }
        return obj.toString()
    }
}
