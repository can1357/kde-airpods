//! `AirPods` protocol definitions and data structures.
//!
//! This module contains all the protocol-specific constants, packet
//! definitions, and data structures for communicating with `AirPods` devices.

use std::{
   fmt,
   num::NonZeroU8,
   str::FromStr,
   sync::{
      LazyLock,
      atomic::{AtomicU64, Ordering},
   },
};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::bluetooth::l2cap::Packet;

pub const PKT_HANDSHAKE: &[u8] = &[
   0x00, 0x00, 0x04, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
pub const PKT_SET_FEATURES: &[u8] = &[
   0x04, 0x00, 0x04, 0x00, 0x4d, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
pub const PKT_REQUEST_NOTIFY: &[u8] = &[
   0x04, 0x00, 0x04, 0x00, 0x0f, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff,
];

// Parsing headers
pub const HDR_BATTERY_STATE: &[u8] = b"\x04\x00\x04\x00\x04\x00";
pub const HDR_NOISE_CTL: &[u8] = b"\x04\x00\x04\x00\x09\x00\x0D";
pub const HDR_CMD_CTL: &[u8] = b"\x04\x00\x04\x00\x09\x00";

// ACK packet headers
pub const HDR_ACK_HANDSHAKE: &[u8] = b"\x01\x00\x04\x00";
pub const HDR_ACK_FEATURES: &[u8] = b"\x04\x00\x04\x00\x2b";
pub const HDR_METADATA: &[u8] = b"\x04\x00\x04\x00\x1d";
pub const HDR_EAR_DETECTION: &[u8] = b"\x04\x00\x04\x00\x06\x00";

/// Represents different components of `AirPods`.
#[repr(u8)]
#[derive(
   Debug,
   Clone,
   Copy,
   PartialEq,
   Eq,
   Serialize,
   Deserialize,
   strum::FromRepr,
   strum::Display,
   strum::EnumString,
)]
pub enum Component {
   Headphone = 0x01,
   Right = 0x02,
   Left = 0x04,
   Case = 0x08,
}

/// Battery status for `AirPods` components.
#[derive(
   Default,
   Debug,
   Clone,
   Copy,
   PartialEq,
   Eq,
   Serialize,
   Deserialize,
   strum::FromRepr,
   strum::Display,
   strum::EnumString,
)]
#[repr(u8)]
pub enum BatteryStatus {
   Normal = 0x00,
   Charging = 0x01,
   Discharging = 0x02,
   #[default]
   Disconnected = 0x04,
}

/// Noise control modes supported by `AirPods`.
#[derive(
   Default,
   Debug,
   Clone,
   Copy,
   PartialEq,
   Eq,
   Serialize,
   Deserialize,
   strum::FromRepr,
   strum::Display,
   strum::EnumString,
   strum::IntoStaticStr,
   strum::EnumIter,
   Hash,
)]
#[repr(u32)]
pub enum NoiseControlMode {
   #[default]
   #[strum(serialize = "off")]
   Off = 0x01,
   #[strum(serialize = "anc")]
   Active = 0x02,
   #[strum(serialize = "transparency")]
   Transparency = 0x03,
   #[strum(serialize = "adaptive")]
   Adaptive = 0x04,
}

impl NoiseControlMode {
   pub fn to_str(self) -> &'static str {
      self.into()
   }

   pub const fn index(self) -> usize {
      (self as usize) - 1 // NOTE: Fix if values change!
   }
   pub fn from_index(i: usize) -> Option<Self> {
      Self::from_repr(i.checked_add(1)?.try_into().ok()?)
   }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct NoiseControlMap<T> {
   data: [Option<T>; 4], // 4 = to_index(NoiseControlMode::MAX) + 1
}

impl<T> Default for NoiseControlMap<T> {
   fn default() -> Self {
      Self {
         data: Default::default(),
      }
   }
}

impl<T: fmt::Debug> fmt::Debug for NoiseControlMap<T> {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      let mut map = f.debug_map();
      for (i, value) in self.data.iter().enumerate() {
         let Some(value) = value else {
            continue;
         };
         map.entry(&NoiseControlMode::from_index(i), &value);
      }
      map.finish()
   }
}

impl<T> NoiseControlMap<T> {
   pub const fn get(&self, mode: NoiseControlMode) -> Option<&T> {
      let i = mode.index();
      self.data[i].as_ref()
   }

   pub const fn insert(&mut self, mode: NoiseControlMode, value: T) -> Option<T> {
      let i = mode.index();
      self.data[i].replace(value)
   }

   pub fn get_or_insert_with(&mut self, mode: NoiseControlMode, f: impl FnOnce() -> T) -> &mut T {
      let i = mode.index();
      self.data[i].get_or_insert_with(f)
   }

   pub const fn remove(&mut self, mode: NoiseControlMode) -> Option<T> {
      let i = mode.index();
      self.data[i].take()
   }

   pub fn len(&self) -> usize {
      self.data.iter().filter(|v| v.is_some()).count()
   }

   #[cfg(test)]
   pub fn is_empty(&self) -> bool {
      self.data.iter().all(|v| v.is_none())
   }
}

pub const KNOWN_FEATURES: &[(u8, &str)] = &[
   (FeatureId::MIC_MODE.id(), "mic_mode"),
   (FeatureId::BUTTON_SEND_MODE.id(), "button_send_mode"),
   (FeatureId::NOISE_CONTROL.id(), "noise_control"),
   (FeatureId::SINGLE_CLICK_MODE.id(), "single_click_mode"),
   (FeatureId::DOUBLE_CLICK_MODE.id(), "double_click_mode"),
   (FeatureId::CLICK_HOLD_MODE.id(), "click_hold_mode"),
   (
      FeatureId::DOUBLE_CLICK_INTERVAL.id(),
      "double_click_interval",
   ),
   (FeatureId::CLICK_HOLD_INTERVAL.id(), "click_hold_interval"),
   (
      FeatureId::LISTENING_MODE_CONFIGS.id(),
      "listening_mode_configs",
   ),
   (FeatureId::ONE_BUD_ANC.id(), "one_bud_anc"),
   (
      FeatureId::CROWN_ROTATION_DIRECTION.id(),
      "crown_rotation_direction",
   ),
   (FeatureId::AUTO_ANSWER_MODE.id(), "auto_answer_mode"),
   (FeatureId::CHIME_VOLUME.id(), "chime_volume"),
   (FeatureId::VOLUME_INTERVAL.id(), "volume_interval"),
   (
      FeatureId::CALL_MANAGEMENT_CONFIG.id(),
      "call_management_config",
   ),
   (FeatureId::VOLUME_SWIPE.id(), "volume_swipe"),
   (FeatureId::ADAPTIVE_VOLUME.id(), "adaptive_volume"),
   (FeatureId::SOFTWARE_MUTE.id(), "software_mute"),
   (FeatureId::CONVERSATIONAL.id(), "conversational"),
   (FeatureId::SSL.id(), "ssl"),
   (FeatureId::HEARING_AID_SETTINGS.id(), "hearing_aid_settings"),
   (FeatureId::AUTO_ANC_STRENGTH.id(), "auto_anc_strength"),
   (FeatureId::HPS_GAIN_SWIPE.id(), "hps_gain_swipe"),
   (FeatureId::HRM.id(), "hrm"),
   (FeatureId::IN_CASE_TONE.id(), "in_case_tone"),
   (FeatureId::SIRI_MULTITONE.id(), "siri_multitone"),
   (FeatureId::HEARING_ASSIST.id(), "hearing_assist"),
   (FeatureId::ALLOW_OFF.id(), "allow_off"),
];

/// Represents a feature command that can be sent to `AirPods`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct FeatureId(u8);

impl FromStr for FeatureId {
   type Err = strum::ParseError;

   fn from_str(s: &str) -> Result<Self, Self::Err> {
      for (repr, name) in KNOWN_FEATURES {
         if name.eq_ignore_ascii_case(s) {
            return Ok(Self(*repr));
         }
      }
      Err(strum::ParseError::VariantNotFound)
   }
}

#[derive(Default)]
pub struct FeatureBitmap([AtomicU64; 256 / 64]);

impl fmt::Debug for FeatureBitmap {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      f.debug_set().entries(self.iter()).finish()
   }
}

impl FeatureBitmap {
   pub fn set(&self, feature: FeatureId, enabled: bool) -> bool {
      let (idx, mask) = feature.bitpos();
      let prev = if enabled {
         self.0[idx].fetch_or(mask, Ordering::Relaxed)
      } else {
         self.0[idx].fetch_and(!mask, Ordering::Relaxed)
      };
      prev & mask != 0
   }
   pub fn get(&self, feature: FeatureId) -> bool {
      let (idx, mask) = feature.bitpos();
      self.0[idx].load(Ordering::Relaxed) & mask != 0
   }
   pub fn iter(&self) -> impl Iterator<Item = FeatureId> {
      (0..=0xffu8).filter_map(move |i| {
         let feature = FeatureId::from_id(i);
         if self.get(feature) {
            Some(feature)
         } else {
            None
         }
      })
   }
}

static U8_TO_HEX: LazyLock<[[u8; 2]; 256]> = LazyLock::new(|| {
   let mut featids = [[0u8; 2]; 256];
   for i in 0..=255u8 {
      const fn nibble_to_hex(n: u8) -> u8 {
         if n < 10 { n + b'0' } else { n - 10 + b'a' }
      }
      featids[i as usize] = [nibble_to_hex(i >> 4), nibble_to_hex(i & 0x0f)];
   }
   featids
});

impl FeatureId {
   // Audio Control
   pub const MIC_MODE: Self = Self(0x01);
   pub const NOISE_CONTROL: Self = Self(0x0D);

   // Button Configuration
   pub const BUTTON_SEND_MODE: Self = Self(0x05);
   pub const SINGLE_CLICK_MODE: Self = Self(0x14);
   pub const DOUBLE_CLICK_MODE: Self = Self(0x15);
   pub const CLICK_HOLD_MODE: Self = Self(0x16);
   pub const DOUBLE_CLICK_INTERVAL: Self = Self(0x17);
   pub const CLICK_HOLD_INTERVAL: Self = Self(0x18);

   // Listening Mode
   pub const LISTENING_MODE_CONFIGS: Self = Self(0x1A);
   pub const ONE_BUD_ANC: Self = Self(0x1B);

   // Crown Control (AirPods Max)
   pub const CROWN_ROTATION_DIRECTION: Self = Self(0x1C);

   // Call Features
   pub const AUTO_ANSWER_MODE: Self = Self(0x1E);
   pub const CALL_MANAGEMENT_CONFIG: Self = Self(0x24);

   // Audio Settings
   pub const CHIME_VOLUME: Self = Self(0x1F);
   pub const VOLUME_INTERVAL: Self = Self(0x23);
   pub const VOLUME_SWIPE: Self = Self(0x25);
   pub const ADAPTIVE_VOLUME: Self = Self(0x26);
   pub const SOFTWARE_MUTE: Self = Self(0x27);

   // Advanced Features
   pub const CONVERSATIONAL: Self = Self(0x28);
   pub const SSL: Self = Self(0x29);
   pub const HEARING_AID_SETTINGS: Self = Self(0x2C);
   pub const AUTO_ANC_STRENGTH: Self = Self(0x2E);
   pub const HPS_GAIN_SWIPE: Self = Self(0x2F);
   pub const HRM: Self = Self(0x30); // Heart Rate Monitor
   pub const IN_CASE_TONE: Self = Self(0x31);
   pub const SIRI_MULTITONE: Self = Self(0x32);
   pub const HEARING_ASSIST: Self = Self(0x33);
   pub const ALLOW_OFF: Self = Self(0x34);

   pub const fn from_id(repr: u8) -> Self {
      Self(repr)
   }

   pub const fn id(self) -> u8 {
      self.0
   }

   pub const fn bitpos(self) -> (usize, u64) {
      let idx = self.0 as usize >> 6;
      let mask = 1 << (self.0 as usize & 0x3f);
      (idx, mask)
   }

   pub fn try_to_str(self) -> Option<&'static str> {
      let Ok(i) = KNOWN_FEATURES.binary_search_by_key(&self.0, |(repr, _)| *repr) else {
         return None;
      };
      let (_, name) = KNOWN_FEATURES[i];
      Some(name)
   }

   pub fn to_str(self) -> &'static str {
      if let Some(name) = self.try_to_str() {
         name
      } else {
         let bytes = &U8_TO_HEX[self.0 as usize];
         str::from_utf8(bytes).unwrap_or("??")
      }
   }
}

impl fmt::Display for FeatureId {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      f.write_str(self.to_str())
   }
}

/// Battery state for a single `AirPods` component.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryState {
   pub level: u8,
   pub status: BatteryStatus,
}

impl fmt::Display for BatteryState {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}%({})", self.level, self.status)
   }
}

impl BatteryState {
   pub const fn new() -> Self {
      Self {
         level: 0,
         status: BatteryStatus::Disconnected,
      }
   }

   pub fn is_charging(self) -> bool {
      self.status == BatteryStatus::Charging
   }

   pub fn is_available(self) -> bool {
      self.status != BatteryStatus::Disconnected
   }

   pub fn to_json(self) -> serde_json::Value {
      if self.is_available() {
         json!({
            "level": u32::from(self.level),
            "charging": self.is_charging(),
         })
      } else {
         serde_json::Value::Null
      }
   }
}

/// Complete battery information for all `AirPods` components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryInfo {
   pub left: BatteryState,
   pub right: BatteryState,
   pub case: BatteryState,
   pub headphone: BatteryState,
}

impl fmt::Display for BatteryInfo {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(
         f,
         "L:{} R:{} C:{} H:{}",
         self.left, self.right, self.case, self.headphone
      )
   }
}

impl BatteryInfo {
   pub const fn new() -> Self {
      Self {
         left: BatteryState::new(),
         right: BatteryState::new(),
         case: BatteryState::new(),
         headphone: BatteryState::new(),
      }
   }

   pub fn split_ref(&self) -> (&BatteryState, &BatteryState) {
      if self.headphone.is_available() {
         (&self.headphone, &self.headphone)
      } else {
         (&self.left, &self.right)
      }
   }

   pub fn to_json(self) -> serde_json::Value {
      json!({
          "left": self.left.to_json(),
          "right": self.right.to_json(),
          "case": self.case.to_json(),
          "headphone": self.headphone.to_json(),
      })
   }
}

/// Ear detection status for left and right `AirPods`.
#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct EarDetectionStatus(NonZeroU8);

impl EarDetectionStatus {
   pub const LEFT: u8 = 1 << 0;
   pub const RIGHT: u8 = 1 << 1;
   pub const VALID: u8 = 0x80;

   pub const fn new(left_in_ear: bool, right_in_ear: bool) -> Self {
      let mut flags = Self::VALID;
      if left_in_ear {
         flags |= Self::LEFT;
      }
      if right_in_ear {
         flags |= Self::RIGHT;
      }
      Self(NonZeroU8::new(flags).expect("(x|valid) != 0"))
   }

   pub const fn is_left_in_ear(&self) -> bool {
      self.0.get() & Self::LEFT != 0
   }
   pub const fn is_right_in_ear(&self) -> bool {
      self.0.get() & Self::RIGHT != 0
   }

   pub fn to_json(self) -> serde_json::Value {
      json!({
          "left_in_ear": self.is_left_in_ear(),
          "right_in_ear": self.is_right_in_ear(),
      })
   }
}

/// Builds a control packet for sending commands to `AirPods`.
pub fn build_control_packet(cmd: u8, data: [u8; 4]) -> Packet {
   HDR_CMD_CTL
      .iter()
      .copied()
      .chain([cmd])
      .chain(data.iter().copied())
      .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FeatureCmd {
   Query = 0,
   Enable = 1,
   Disable = 2,
}

impl FeatureCmd {
   pub fn build(self, feature: u8) -> Packet {
      let data = self as u32;
      build_control_packet(feature, data.to_le_bytes())
   }
   pub fn parse(data: &[u8]) -> Option<(FeatureId, Self)> {
      let rest = data.strip_prefix(HDR_CMD_CTL)?;
      let (feature, rest) = rest.split_first()?;
      let u: u32 = u32::from_le_bytes(rest.try_into().ok()?);
      match u {
         0 => Some((FeatureId::from_id(*feature), Self::Query)),
         1 => Some((FeatureId::from_id(*feature), Self::Enable)),
         2 => Some((FeatureId::from_id(*feature), Self::Disable)),
         _ => None,
      }
   }
}
