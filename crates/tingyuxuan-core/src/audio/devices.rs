//! 音频输入设备枚举与解析。
//!
//! 本模块封装 cpal 设备管理 API，提供：
//! - `enumerate_input_devices()` — 列出所有可用音频输入设备
//! - `resolve_input_device()` — 根据持久化 DeviceId 查找设备（fallback 到默认）
//!
//! Mock 模式（`TINGYUXUAN_MOCK_AUDIO=1`）下返回模拟设备，无需真实硬件。

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

use crate::error::AudioError;

/// 音频输入设备信息 — 用于 UI 展示和持久化选择。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    /// 设备唯一标识（`DeviceId.to_string()`），跨重启稳定，用于持久化。
    pub id: String,
    /// 用户可读名称（`DeviceDescription`），用于 UI 显示。
    pub name: String,
    /// 是否为系统默认输入设备。
    pub is_default: bool,
}

/// 枚举所有可用音频输入设备。
///
/// Mock 模式下返回一个 "Mock Microphone" 虚拟设备。
pub fn enumerate_input_devices() -> Result<Vec<AudioDeviceInfo>, AudioError> {
    if is_mock_mode() {
        return Ok(vec![AudioDeviceInfo {
            id: "mock-default".to_string(),
            name: "Mock Microphone".to_string(),
            is_default: true,
        }]);
    }

    let host = cpal::default_host();
    let default_id = host
        .default_input_device()
        .and_then(|d| d.id().ok())
        .map(|id| id.to_string());

    let mut devices = Vec::new();
    for device in host.input_devices().map_err(|e| {
        AudioError::StreamError(format!("无法枚举输入设备: {e}"))
    })? {
        let id = match device.id() {
            Ok(id) => id.to_string(),
            Err(_) => continue, // 跳过无法获取 ID 的设备
        };
        let name = device
            .description()
            .map(|d| d.to_string())
            .unwrap_or_else(|_| "Unknown Device".to_string());
        let is_default = default_id.as_deref() == Some(&id);
        devices.push(AudioDeviceInfo { id, name, is_default });
    }

    Ok(devices)
}

/// 根据持久化的 DeviceId 字符串查找输入设备。
///
/// - `device_id = None` → 返回系统默认输入设备
/// - `device_id = Some(id)` → 尝试精确匹配；失败时 fallback 到默认设备 + warn 日志
pub fn resolve_input_device(
    device_id: Option<&str>,
) -> Result<cpal::Device, AudioError> {
    let host = cpal::default_host();

    let Some(target_id) = device_id else {
        return host
            .default_input_device()
            .ok_or(AudioError::NoInputDevice);
    };

    // 尝试通过 DeviceId 精确查找。
    let parsed_id: cpal::DeviceId = target_id.parse().map_err(|_| {
        AudioError::StreamError(format!("无效的设备 ID: {target_id}"))
    })?;

    if let Some(device) = host.device_by_id(&parsed_id) {
        return Ok(device);
    }

    // Fallback：设备可能已拔出，回退到默认设备。
    tracing::warn!(
        device_id = target_id,
        "指定音频设备未找到，回退到系统默认设备"
    );
    host.default_input_device()
        .ok_or(AudioError::NoInputDevice)
}

fn is_mock_mode() -> bool {
    std::env::var("TINGYUXUAN_MOCK_AUDIO")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_mock_mode() {
        temp_env::with_var("TINGYUXUAN_MOCK_AUDIO", Some("1"), || {
            let devices = enumerate_input_devices().unwrap();
            assert_eq!(devices.len(), 1);
            assert_eq!(devices[0].name, "Mock Microphone");
        });
    }

    #[test]
    fn test_mock_device_is_default() {
        temp_env::with_var("TINGYUXUAN_MOCK_AUDIO", Some("1"), || {
            let devices = enumerate_input_devices().unwrap();
            assert!(devices[0].is_default);
        });
    }

    #[test]
    fn test_audio_device_info_serialization() {
        let info = AudioDeviceInfo {
            id: "test-id-123".to_string(),
            name: "Test Microphone".to_string(),
            is_default: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        let roundtrip: AudioDeviceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.id, "test-id-123");
        assert_eq!(roundtrip.name, "Test Microphone");
        assert!(roundtrip.is_default);
    }
}
