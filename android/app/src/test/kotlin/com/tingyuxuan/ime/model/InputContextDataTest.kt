package com.tingyuxuan.ime.model

import org.json.JSONObject
import org.junit.Test
import org.junit.Assert.*

/**
 * InputContextData 单元测试 — 验证 JSON 序列化与 Rust serde 字段名一致。
 */
class InputContextDataTest {

    @Test
    fun `empty context produces empty JSON object`() {
        val ctx = InputContextData()
        val json = ctx.toJson()
        val obj = JSONObject(json)
        assertEquals(0, obj.length())
    }

    @Test
    fun `toJson uses snake_case field names`() {
        val ctx = InputContextData(
            appName = "微信",
            appPackage = "com.tencent.mm",
            inputFieldType = "chat",
            inputHint = "发送消息",
            editorAction = "send",
            surroundingText = "你好",
            selectedText = "世界",
            clipboardText = "剪贴板内容",
        )
        val obj = JSONObject(ctx.toJson())

        assertEquals("微信", obj.getString("app_name"))
        assertEquals("com.tencent.mm", obj.getString("app_package"))
        assertEquals("chat", obj.getString("input_field_type"))
        assertEquals("发送消息", obj.getString("input_hint"))
        assertEquals("send", obj.getString("editor_action"))
        assertEquals("你好", obj.getString("surrounding_text"))
        assertEquals("世界", obj.getString("selected_text"))
        assertEquals("剪贴板内容", obj.getString("clipboard_text"))
    }

    @Test
    fun `null fields are omitted from JSON`() {
        val ctx = InputContextData(appName = "Chrome", browserUrl = null)
        val obj = JSONObject(ctx.toJson())

        assertTrue(obj.has("app_name"))
        assertFalse(obj.has("browser_url"))
        assertFalse(obj.has("window_title"))
        assertFalse(obj.has("selected_text"))
    }

    @Test
    fun `all fields serialize correctly`() {
        val ctx = InputContextData(
            appName = "App",
            appPackage = "com.example",
            windowTitle = "Window",
            browserUrl = "https://example.com",
            inputFieldType = "email",
            inputHint = "Enter email",
            editorAction = "done",
            surroundingText = "surrounding",
            selectedText = "selected",
            clipboardText = "clipboard",
            screenText = "screen",
        )
        val obj = JSONObject(ctx.toJson())
        assertEquals(11, obj.length())
        assertEquals("App", obj.getString("app_name"))
        assertEquals("com.example", obj.getString("app_package"))
        assertEquals("Window", obj.getString("window_title"))
        assertEquals("https://example.com", obj.getString("browser_url"))
        assertEquals("email", obj.getString("input_field_type"))
        assertEquals("Enter email", obj.getString("input_hint"))
        assertEquals("done", obj.getString("editor_action"))
        assertEquals("surrounding", obj.getString("surrounding_text"))
        assertEquals("selected", obj.getString("selected_text"))
        assertEquals("clipboard", obj.getString("clipboard_text"))
        assertEquals("screen", obj.getString("screen_text"))
    }

    @Test
    fun `special characters in values are preserved`() {
        val ctx = InputContextData(
            appName = "App \"with\" quotes",
            surroundingText = "line1\nline2\ttab",
        )
        val json = ctx.toJson()
        val obj = JSONObject(json)

        assertEquals("App \"with\" quotes", obj.getString("app_name"))
        assertTrue(obj.getString("surrounding_text").contains("\n"))
        assertTrue(obj.getString("surrounding_text").contains("\t"))
    }

    @Test
    fun `data class copy works correctly`() {
        val original = InputContextData(appName = "App1")
        val modified = original.copy(appName = "App2", inputFieldType = "chat")

        assertEquals("App2", modified.appName)
        assertEquals("chat", modified.inputFieldType)
        assertNull(modified.appPackage)
    }
}
