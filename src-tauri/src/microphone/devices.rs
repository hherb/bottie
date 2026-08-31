//! Session-only microphone discovery and opaque input selection.

use cpal::{
    DeviceId,
    traits::{DeviceTrait, HostTrait},
};
use serde::Serialize;

use super::{MicrophoneController, lock};

pub(super) const MAX_INPUT_DEVICES: usize = 64;
const MAX_INPUT_LABEL_BYTES: usize = 160;
const MAX_NATIVE_DEVICE_ID_BYTES: usize = 2_048;
pub(super) const SYSTEM_DEFAULT_INPUT_TOKEN: &str = "system-default";

/// One bounded display choice without a native device identifier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MicrophoneInputDevice {
    pub(super) token: String,
    pub(super) label: String,
    pub(super) is_system_default: bool,
}

/// Current process-only microphone choices and selected opaque token.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MicrophoneInputDeviceList {
    pub(super) devices: Vec<MicrophoneInputDevice>,
    pub(super) selected_token: String,
    pub(super) selection_available: bool,
}

/// Stable path-free errors for explicit microphone discovery and selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MicrophoneDeviceCommandError {
    /// Device discovery or selection cannot change while native capture is active.
    CaptureActive,
    /// The local audio host could not enumerate its current input devices.
    DiscoveryFailed,
    /// The submitted opaque token is not one of the current bounded choices.
    SelectionNotFound,
}

/// Exact native input selected behind one process-local public token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CaptureInputSelection {
    /// Resolve the operating system's current default only when capture starts.
    SystemDefault,
    /// Resolve this exact native identity only when capture starts.
    Exact(DeviceId),
}

/// Native-only discovery record that is never serialized to the WebView.
#[derive(Clone, Debug)]
pub(super) struct NativeInputDevice {
    pub(super) id: DeviceId,
    pub(super) label: String,
}

/// Process-lifetime registry for bounded choices and one session-only selection.
pub(super) struct MicrophoneDeviceRegistry {
    entries: Vec<(MicrophoneInputDevice, DeviceId)>,
    next_token: u64,
    selected_token: String,
    selected: CaptureInputSelection,
}

impl Default for MicrophoneDeviceRegistry {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_token: 1,
            selected_token: SYSTEM_DEFAULT_INPUT_TOKEN.into(),
            selected: CaptureInputSelection::SystemDefault,
        }
    }
}

impl MicrophoneDeviceRegistry {
    /// Replaces current discovery results while retaining a missing exact selection as stale.
    pub(super) fn refresh(
        &mut self,
        candidates: Vec<NativeInputDevice>,
    ) -> MicrophoneInputDeviceList {
        let previous_entries = std::mem::take(&mut self.entries);
        self.entries = bounded_input_devices(candidates)
            .into_iter()
            .map(|(label, native_id)| {
                let token = previous_entries
                    .iter()
                    .find_map(|(device, previous_id)| {
                        (previous_id == &native_id).then(|| device.token.clone())
                    })
                    .unwrap_or_else(|| {
                        let token = format!("local-input-{number:016x}", number = self.next_token);
                        self.next_token = self
                            .next_token
                            .checked_add(1)
                            .expect("a process cannot enumerate every u64 microphone token");
                        token
                    });
                (
                    MicrophoneInputDevice {
                        token,
                        label,
                        is_system_default: false,
                    },
                    native_id,
                )
            })
            .collect();
        if let CaptureInputSelection::Exact(selected_id) = &self.selected
            && let Some((device, _)) = self
                .entries
                .iter()
                .find(|(_, native_id)| native_id == selected_id)
        {
            self.selected_token = device.token.clone();
        }
        self.list()
    }

    /// Selects one current opaque token without exposing its native mapping.
    pub(super) fn select(&mut self, token: &str) -> Result<(), MicrophoneDeviceCommandError> {
        if token == SYSTEM_DEFAULT_INPUT_TOKEN {
            self.selected_token = token.into();
            self.selected = CaptureInputSelection::SystemDefault;
            return Ok(());
        }
        let (_, native_id) = self
            .entries
            .iter()
            .find(|(device, _)| device.token == token)
            .ok_or(MicrophoneDeviceCommandError::SelectionNotFound)?;
        self.selected_token = token.into();
        self.selected = CaptureInputSelection::Exact(native_id.clone());
        Ok(())
    }

    /// Returns the native-only selection snapshot used by one explicit capture start.
    pub(super) fn capture_selection(&self) -> CaptureInputSelection {
        self.selected.clone()
    }

    fn list(&self) -> MicrophoneInputDeviceList {
        let mut devices = vec![MicrophoneInputDevice {
            token: SYSTEM_DEFAULT_INPUT_TOKEN.into(),
            label: "System default".into(),
            is_system_default: true,
        }];
        devices.extend(self.entries.iter().map(|(device, _)| device.clone()));
        let selection_available = match &self.selected {
            CaptureInputSelection::SystemDefault => true,
            CaptureInputSelection::Exact(selected_id) => self
                .entries
                .iter()
                .any(|(_, native_id)| native_id == selected_id),
        };
        MicrophoneInputDeviceList {
            devices,
            selected_token: self.selected_token.clone(),
            selection_available,
        }
    }
}

impl MicrophoneController {
    /// Lazily enumerates bounded input choices only after an explicit WebView action.
    pub(crate) fn list_input_devices(
        &self,
    ) -> Result<MicrophoneInputDeviceList, MicrophoneDeviceCommandError> {
        let mut devices = lock(&self.devices);
        if self.is_capturing() {
            return Err(MicrophoneDeviceCommandError::CaptureActive);
        }
        let candidates = discover_input_devices()?;
        Ok(devices.refresh(candidates))
    }

    /// Selects one current opaque microphone token for this process lifetime.
    pub(crate) fn select_input_device(
        &self,
        token: &str,
    ) -> Result<MicrophoneInputDeviceList, MicrophoneDeviceCommandError> {
        let mut devices = lock(&self.devices);
        if self.is_capturing() {
            return Err(MicrophoneDeviceCommandError::CaptureActive);
        }
        devices.select(token)?;
        Ok(devices.list())
    }
}

fn discover_input_devices() -> Result<Vec<NativeInputDevice>, MicrophoneDeviceCommandError> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|_| MicrophoneDeviceCommandError::DiscoveryFailed)?;
    Ok(devices
        .filter_map(|device| {
            let id = device.id().ok()?;
            Some(NativeInputDevice {
                id,
                label: device.to_string(),
            })
        })
        .collect())
}

fn bounded_input_devices(candidates: Vec<NativeInputDevice>) -> Vec<(String, DeviceId)> {
    let mut candidates = candidates
        .into_iter()
        .filter_map(|candidate| {
            let native_key = candidate.id.to_string();
            let label = bounded_device_label(&candidate.label);
            (!native_key.is_empty()
                && native_key.len() <= MAX_NATIVE_DEVICE_ID_BYTES
                && !label.is_empty())
            .then_some((native_key, label, candidate.id))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0));
    candidates.dedup_by(|left, right| left.0 == right.0);
    candidates.sort_by(|left, right| (&left.1, &left.0).cmp(&(&right.1, &right.0)));
    candidates.truncate(MAX_INPUT_DEVICES);
    candidates
        .into_iter()
        .map(|(_, label, native_id)| (label, native_id))
        .collect()
}

fn bounded_device_label(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let mut end = normalized.len().min(MAX_INPUT_LABEL_BYTES);
    while !normalized.is_char_boundary(end) {
        end -= 1;
    }
    normalized[..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::microphone::{MicrophoneErrorCode, capture::resolve_input_device_from_candidates};

    fn test_device_id(value: &str) -> DeviceId {
        DeviceId::new(cpal::default_host().id(), value)
    }

    #[test]
    fn bounds_and_sanitizes_input_devices_without_serializing_native_identity() {
        let mut candidates = (0..MAX_INPUT_DEVICES + 8)
            .map(|index| NativeInputDevice {
                id: test_device_id(&format!("native-device-{index}")),
                label: format!(" Microphone {index:03} "),
            })
            .collect::<Vec<_>>();
        candidates.push(NativeInputDevice {
            id: test_device_id("native-secret-control"),
            label: format!("Unsafe\0\n{}", "é".repeat(200)),
        });

        let mut registry = MicrophoneDeviceRegistry::default();
        let list = registry.refresh(candidates);

        assert_eq!(list.devices.len(), MAX_INPUT_DEVICES + 1);
        assert_eq!(list.devices[0].token, SYSTEM_DEFAULT_INPUT_TOKEN);
        assert_eq!(list.devices[0].label, "System default");
        assert!(list.devices[0].is_system_default);
        assert!(list.devices.iter().skip(1).all(|device| {
            !device.token.contains("native")
                && !device.label.chars().any(char::is_control)
                && device.label.len() <= MAX_INPUT_LABEL_BYTES
        }));

        let serialized = serde_json::to_value(&list).unwrap();
        assert_eq!(
            serialized["devices"][0]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            ["isSystemDefault", "label", "token"],
        );
        let serialized = serialized.to_string();
        assert!(!serialized.contains("native-device"));
        assert!(!serialized.to_ascii_lowercase().contains("host"));
        assert!(!serialized.to_ascii_lowercase().contains("path"));
    }

    #[test]
    fn opaque_selection_resolves_only_to_the_exact_native_device_and_stays_stale() {
        let exact_id = test_device_id("exact-native-device");
        let mut registry = MicrophoneDeviceRegistry::default();
        let listed = registry.refresh(vec![NativeInputDevice {
            id: exact_id.clone(),
            label: "Desk microphone".into(),
        }]);
        let exact_token = listed.devices[1].token.clone();

        registry.select(&exact_token).unwrap();
        assert_eq!(
            registry.capture_selection(),
            CaptureInputSelection::Exact(exact_id.clone()),
        );
        assert_eq!(
            resolve_input_device_from_candidates(
                &registry.capture_selection(),
                Some("default microphone"),
                vec![(exact_id.clone(), "desk microphone")],
            ),
            Ok("desk microphone"),
        );

        let refreshed = registry.refresh(vec![NativeInputDevice {
            id: test_device_id("replacement-native-device"),
            label: "Replacement microphone".into(),
        }]);
        assert_eq!(refreshed.selected_token, exact_token);
        assert!(!refreshed.selection_available);
        assert_ne!(refreshed.devices[1].token, exact_token);
        assert_eq!(
            resolve_input_device_from_candidates(
                &registry.capture_selection(),
                Some("different default"),
                Vec::<(DeviceId, &str)>::new(),
            ),
            Err(MicrophoneErrorCode::SelectedDeviceUnavailable),
        );
        assert_eq!(registry.select(SYSTEM_DEFAULT_INPUT_TOKEN), Ok(()));
        assert_eq!(
            registry.capture_selection(),
            CaptureInputSelection::SystemDefault,
        );
    }
}
