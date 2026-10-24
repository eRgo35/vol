//! Pure decoders for node and device param pods (`docs/specs/01-architecture.md` §5,
//! VOL-ARCH-053).
//!
//! Each decoder takes one param pod as delivered by a `param` listener and returns the
//! model value or a [`DecodeError`]; the caller skips a param that fails to decode.
//! A property with an unexpected pod type is an error. Absent properties take these
//! values: `Props` `volume` 1.0, `mute` false, `channelVolumes`/`channelMap` empty;
//! profile/route `description` empty, `priority` 0, `available` unknown, route
//! `profiles`/`devices` empty; `ProcessLatency` `ns` 0 (as `spa/param/latency-utils.h`
//! parses it). Every other property listed per decoder is required.

use pipewire::spa::pod::deserialize::PodDeserializer;
use pipewire::spa::pod::{Object, Pod, Value, ValueArray};
use pipewire::spa::sys;
use pipewire::spa::utils::SpaTypes;

use crate::model::{ActiveRoute, Availability, Channel, NodeAudio, PortDir, Profile, RouteInfo};

/// Why a param pod could not be decoded.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodeError {
    /// The bytes are not a well-formed pod.
    Malformed,
    /// The pod is not an object of the expected object type.
    WrongObject,
    /// A required property is absent.
    Missing(&'static str),
    /// A property has an unexpected pod type.
    WrongType(&'static str),
}

/// Decodes a node `Props` param: `volume`, `channelVolumes`, `mute`, `channelMap`.
pub fn decode_props(pod: &Pod) -> Result<NodeAudio, DecodeError> {
    let obj = object(pod, SpaTypes::ObjectParamProps)?;
    Ok(NodeAudio {
        volume: field(&obj, sys::SPA_PROP_volume, "volume", float)?.unwrap_or(1.0),
        channel_volumes: field(&obj, sys::SPA_PROP_channelVolumes, "channelVolumes", floats)?
            .unwrap_or_default(),
        mute: field(&obj, sys::SPA_PROP_mute, "mute", boolean)?.unwrap_or(false),
        channel_map: field(&obj, sys::SPA_PROP_channelMap, "channelMap", ids)?
            .unwrap_or_default()
            .into_iter()
            .map(Channel)
            .collect(),
    })
}

/// Decodes one device `EnumProfile` param object; `index` and `name` are required.
pub fn decode_enum_profile(pod: &Pod) -> Result<Profile, DecodeError> {
    let obj = object(pod, SpaTypes::ObjectParamProfile)?;
    Ok(Profile {
        index: required(&obj, sys::SPA_PARAM_PROFILE_index, "index", int)?,
        name: required(&obj, sys::SPA_PARAM_PROFILE_name, "name", string)?,
        description: field(
            &obj,
            sys::SPA_PARAM_PROFILE_description,
            "description",
            string,
        )?
        .unwrap_or_default(),
        priority: field(&obj, sys::SPA_PARAM_PROFILE_priority, "priority", int)?.unwrap_or(0),
        available: availability(&obj, sys::SPA_PARAM_PROFILE_available)?,
    })
}

/// Decodes a device `Profile` param into the active profile `index`.
pub fn decode_profile_index(pod: &Pod) -> Result<i32, DecodeError> {
    let obj = object(pod, SpaTypes::ObjectParamProfile)?;
    required(&obj, sys::SPA_PARAM_PROFILE_index, "index", int)
}

/// Decodes one device `EnumRoute` param object; `index`, `direction` and `name` are
/// required.
pub fn decode_enum_route(pod: &Pod) -> Result<RouteInfo, DecodeError> {
    let obj = object(pod, SpaTypes::ObjectParamRoute)?;
    Ok(RouteInfo {
        index: required(&obj, sys::SPA_PARAM_ROUTE_index, "index", int)?,
        dir: required(&obj, sys::SPA_PARAM_ROUTE_direction, "direction", direction)?,
        name: required(&obj, sys::SPA_PARAM_ROUTE_name, "name", string)?,
        description: field(
            &obj,
            sys::SPA_PARAM_ROUTE_description,
            "description",
            string,
        )?
        .unwrap_or_default(),
        priority: field(&obj, sys::SPA_PARAM_ROUTE_priority, "priority", int)?.unwrap_or(0),
        available: availability(&obj, sys::SPA_PARAM_ROUTE_available)?,
        profiles: field(&obj, sys::SPA_PARAM_ROUTE_profiles, "profiles", ints)?.unwrap_or_default(),
        devices: field(&obj, sys::SPA_PARAM_ROUTE_devices, "devices", ints)?.unwrap_or_default(),
    })
}

/// Decodes one device `Route` param object (an active route); `index`, `device` and
/// `direction` are required.
pub fn decode_route(pod: &Pod) -> Result<ActiveRoute, DecodeError> {
    let obj = object(pod, SpaTypes::ObjectParamRoute)?;
    Ok(ActiveRoute {
        index: required(&obj, sys::SPA_PARAM_ROUTE_index, "index", int)?,
        card_profile_device: required(&obj, sys::SPA_PARAM_ROUTE_device, "device", int)?,
        dir: required(&obj, sys::SPA_PARAM_ROUTE_direction, "direction", direction)?,
    })
}

/// Decodes a node `ProcessLatency` param into its `ns` value.
pub fn decode_process_latency(pod: &Pod) -> Result<i64, DecodeError> {
    let obj = object(pod, SpaTypes::ObjectParamProcessLatency)?;
    Ok(field(&obj, sys::SPA_PARAM_PROCESS_LATENCY_ns, "ns", long)?.unwrap_or(0))
}

/// Parses `pod` as an object of `type_`.
fn object(pod: &Pod, type_: SpaTypes) -> Result<Object, DecodeError> {
    if !pod.is_object() {
        return Err(DecodeError::WrongObject);
    }
    match PodDeserializer::deserialize_from::<Value>(pod.as_bytes()) {
        Ok((_, Value::Object(obj))) if obj.type_ == type_.as_raw() => Ok(obj),
        Ok(_) => Err(DecodeError::WrongObject),
        Err(_) => Err(DecodeError::Malformed),
    }
}

/// Property `key` of `obj` converted by `convert`: `None` if absent, an error if
/// `convert` rejects its type.
fn field<T>(
    obj: &Object,
    key: u32,
    name: &'static str,
    convert: fn(&Value) -> Option<T>,
) -> Result<Option<T>, DecodeError> {
    obj.properties
        .iter()
        .find(|p| p.key == key)
        .map(|p| convert(&p.value).ok_or(DecodeError::WrongType(name)))
        .transpose()
}

/// Like [`field`], but an absent property is an error.
fn required<T>(
    obj: &Object,
    key: u32,
    name: &'static str,
    convert: fn(&Value) -> Option<T>,
) -> Result<T, DecodeError> {
    field(obj, key, name, convert)?.ok_or(DecodeError::Missing(name))
}

fn availability(obj: &Object, key: u32) -> Result<Availability, DecodeError> {
    Ok(field(obj, key, "available", id)?.map_or(Availability::Unknown, Availability::from_spa))
}

fn int(v: &Value) -> Option<i32> {
    match v {
        Value::Int(i) => Some(*i),
        _ => None,
    }
}

fn long(v: &Value) -> Option<i64> {
    match v {
        Value::Long(l) => Some(*l),
        _ => None,
    }
}

fn float(v: &Value) -> Option<f32> {
    match v {
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn boolean(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

fn id(v: &Value) -> Option<u32> {
    match v {
        Value::Id(id) => Some(id.0),
        _ => None,
    }
}

fn string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn floats(v: &Value) -> Option<Vec<f32>> {
    match v {
        Value::ValueArray(ValueArray::Float(f)) => Some(f.clone()),
        _ => None,
    }
}

fn ints(v: &Value) -> Option<Vec<i32>> {
    match v {
        Value::ValueArray(ValueArray::Int(i)) => Some(i.clone()),
        _ => None,
    }
}

fn ids(v: &Value) -> Option<Vec<u32>> {
    match v {
        Value::ValueArray(ValueArray::Id(ids)) => Some(ids.iter().map(|id| id.0).collect()),
        _ => None,
    }
}

/// `SPA_DIRECTION_INPUT` is a capture route (`In`), `SPA_DIRECTION_OUTPUT` a playback
/// route (`Out`).
fn direction(v: &Value) -> Option<PortDir> {
    match id(v)? {
        sys::SPA_DIRECTION_INPUT => Some(PortDir::In),
        sys::SPA_DIRECTION_OUTPUT => Some(PortDir::Out),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use pipewire::spa::param::ParamType;
    use pipewire::spa::pod::serialize::PodSerializer;
    use pipewire::spa::pod::{Object, Property, Value, ValueArray};
    use pipewire::spa::sys;
    use pipewire::spa::utils::{Id, SpaTypes};

    use super::*;

    /// Serialized bytes of a param object.
    fn object(type_: SpaTypes, id: ParamType, properties: Vec<Property>) -> Vec<u8> {
        let value = Value::Object(Object {
            type_: type_.as_raw(),
            id: id.as_raw(),
            properties,
        });
        PodSerializer::serialize(Cursor::new(Vec::new()), &value)
            .unwrap()
            .0
            .into_inner()
    }

    fn pod(bytes: &[u8]) -> &Pod {
        Pod::from_bytes(bytes).unwrap()
    }

    fn prop(key: u32, value: Value) -> Property {
        Property::new(key, value)
    }

    fn floats(v: &[f32]) -> Value {
        Value::ValueArray(ValueArray::Float(v.to_vec()))
    }

    fn ints(v: &[i32]) -> Value {
        Value::ValueArray(ValueArray::Int(v.to_vec()))
    }

    fn props(properties: Vec<Property>) -> Vec<u8> {
        object(SpaTypes::ObjectParamProps, ParamType::Props, properties)
    }

    fn stereo_props() -> Vec<u8> {
        props(vec![
            prop(sys::SPA_PROP_volume, Value::Float(1.0)),
            prop(sys::SPA_PROP_mute, Value::Bool(false)),
            prop(sys::SPA_PROP_channelVolumes, floats(&[0.658492; 2])),
            prop(
                sys::SPA_PROP_channelMap,
                Value::ValueArray(ValueArray::Id(vec![
                    Id(sys::SPA_AUDIO_CHANNEL_FL),
                    Id(sys::SPA_AUDIO_CHANNEL_FR),
                ])),
            ),
        ])
    }

    fn enum_profile(index: i32, available: u32) -> Vec<u8> {
        object(
            SpaTypes::ObjectParamProfile,
            ParamType::EnumProfile,
            vec![
                prop(sys::SPA_PARAM_PROFILE_index, Value::Int(index)),
                prop(
                    sys::SPA_PARAM_PROFILE_name,
                    Value::String("output:analog-stereo".into()),
                ),
                prop(
                    sys::SPA_PARAM_PROFILE_description,
                    Value::String("Analog Stereo Output".into()),
                ),
                prop(sys::SPA_PARAM_PROFILE_priority, Value::Int(6500)),
                prop(sys::SPA_PARAM_PROFILE_available, Value::Id(Id(available))),
            ],
        )
    }

    fn enum_route(index: i32, direction: u32, name: &str) -> Vec<u8> {
        object(
            SpaTypes::ObjectParamRoute,
            ParamType::EnumRoute,
            vec![
                prop(sys::SPA_PARAM_ROUTE_index, Value::Int(index)),
                prop(sys::SPA_PARAM_ROUTE_direction, Value::Id(Id(direction))),
                prop(sys::SPA_PARAM_ROUTE_name, Value::String(name.into())),
                prop(
                    sys::SPA_PARAM_ROUTE_description,
                    Value::String(format!("{name} port")),
                ),
                prop(sys::SPA_PARAM_ROUTE_priority, Value::Int(9000)),
                prop(
                    sys::SPA_PARAM_ROUTE_available,
                    Value::Id(Id(sys::SPA_PARAM_AVAILABILITY_yes)),
                ),
                prop(sys::SPA_PARAM_ROUTE_profiles, ints(&[1, 3])),
                prop(sys::SPA_PARAM_ROUTE_devices, ints(&[0, 2])),
            ],
        )
    }

    #[test]
    fn props_roundtrip_stereo() {
        let bytes = stereo_props();
        let audio = decode_props(pod(&bytes)).unwrap();
        assert_eq!(
            audio,
            NodeAudio {
                volume: 1.0,
                channel_volumes: vec![0.658492; 2],
                mute: false,
                channel_map: vec![Channel(3), Channel(4)],
            }
        );
    }

    #[test]
    fn props_missing_volume_defaults_to_one() {
        let bytes = props(vec![
            prop(sys::SPA_PROP_channelVolumes, floats(&[0.5])),
            prop(sys::SPA_PROP_mute, Value::Bool(true)),
        ]);
        let audio = decode_props(pod(&bytes)).unwrap();
        assert_eq!(audio.volume, 1.0);
        assert_eq!(audio.channel_volumes, vec![0.5]);
        assert!(audio.mute);
        assert!(audio.channel_map.is_empty());
    }

    #[test]
    fn props_wrong_type_is_error() {
        let volume_int = props(vec![prop(sys::SPA_PROP_volume, Value::Int(1))]);
        assert_eq!(
            decode_props(pod(&volume_int)),
            Err(DecodeError::WrongType("volume"))
        );
        let volumes_int = props(vec![prop(sys::SPA_PROP_channelVolumes, ints(&[1, 1]))]);
        assert_eq!(
            decode_props(pod(&volumes_int)),
            Err(DecodeError::WrongType("channelVolumes"))
        );
        let mute_float = props(vec![prop(sys::SPA_PROP_mute, Value::Float(0.0))]);
        assert_eq!(
            decode_props(pod(&mute_float)),
            Err(DecodeError::WrongType("mute"))
        );
        let map_floats = props(vec![prop(sys::SPA_PROP_channelMap, floats(&[3.0]))]);
        assert_eq!(
            decode_props(pod(&map_floats)),
            Err(DecodeError::WrongType("channelMap"))
        );
        let not_props = enum_profile(1, sys::SPA_PARAM_AVAILABILITY_yes);
        assert_eq!(decode_props(pod(&not_props)), Err(DecodeError::WrongObject));
        let not_object = PodSerializer::serialize(Cursor::new(Vec::new()), &Value::Int(1))
            .unwrap()
            .0
            .into_inner();
        assert_eq!(
            decode_props(pod(&not_object)),
            Err(DecodeError::WrongObject)
        );
    }

    #[test]
    fn props_empty_channel_volumes_ok_len_zero() {
        let bytes = props(vec![
            prop(sys::SPA_PROP_volume, Value::Float(0.5)),
            prop(sys::SPA_PROP_channelVolumes, floats(&[])),
        ]);
        let audio = decode_props(pod(&bytes)).unwrap();
        assert_eq!(audio.volume, 0.5);
        assert!(audio.channel_volumes.is_empty());
        assert!(!audio.mute);
    }

    #[test]
    fn enum_profile_fields() {
        let bytes = enum_profile(1, sys::SPA_PARAM_AVAILABILITY_yes);
        assert_eq!(
            decode_enum_profile(pod(&bytes)).unwrap(),
            Profile {
                index: 1,
                name: "output:analog-stereo".into(),
                description: "Analog Stereo Output".into(),
                priority: 6500,
                available: Availability::Yes,
            }
        );
        for (spa, expected) in [
            (sys::SPA_PARAM_AVAILABILITY_no, Availability::No),
            (sys::SPA_PARAM_AVAILABILITY_unknown, Availability::Unknown),
        ] {
            let bytes = enum_profile(2, spa);
            assert_eq!(
                decode_enum_profile(pod(&bytes)).unwrap().available,
                expected
            );
        }
        let no_index = object(
            SpaTypes::ObjectParamProfile,
            ParamType::EnumProfile,
            vec![prop(
                sys::SPA_PARAM_PROFILE_name,
                Value::String("off".into()),
            )],
        );
        assert_eq!(
            decode_enum_profile(pod(&no_index)),
            Err(DecodeError::Missing("index"))
        );
    }

    #[test]
    fn profile_index() {
        let bytes = object(
            SpaTypes::ObjectParamProfile,
            ParamType::Profile,
            vec![
                prop(sys::SPA_PARAM_PROFILE_index, Value::Int(3)),
                prop(sys::SPA_PARAM_PROFILE_save, Value::Bool(true)),
            ],
        );
        assert_eq!(decode_profile_index(pod(&bytes)), Ok(3));
        let wrong = object(
            SpaTypes::ObjectParamProfile,
            ParamType::Profile,
            vec![prop(sys::SPA_PARAM_PROFILE_index, Value::Long(3))],
        );
        assert_eq!(
            decode_profile_index(pod(&wrong)),
            Err(DecodeError::WrongType("index"))
        );
    }

    #[test]
    fn enum_route_input_and_output_directions_with_devices_and_profiles() {
        let out = enum_route(2, sys::SPA_DIRECTION_OUTPUT, "analog-output-speaker");
        assert_eq!(
            decode_enum_route(pod(&out)).unwrap(),
            RouteInfo {
                index: 2,
                dir: PortDir::Out,
                name: "analog-output-speaker".into(),
                description: "analog-output-speaker port".into(),
                priority: 9000,
                available: Availability::Yes,
                profiles: vec![1, 3],
                devices: vec![0, 2],
            }
        );
        let input = enum_route(0, sys::SPA_DIRECTION_INPUT, "analog-input-mic");
        let route = decode_enum_route(pod(&input)).unwrap();
        assert_eq!(route.dir, PortDir::In);
        assert_eq!(route.index, 0);
        let bad_dir = enum_route(0, 7, "x");
        assert_eq!(
            decode_enum_route(pod(&bad_dir)),
            Err(DecodeError::WrongType("direction"))
        );
    }

    #[test]
    fn route_active_index_device_direction() {
        let bytes = object(
            SpaTypes::ObjectParamRoute,
            ParamType::Route,
            vec![
                prop(sys::SPA_PARAM_ROUTE_index, Value::Int(2)),
                prop(
                    sys::SPA_PARAM_ROUTE_direction,
                    Value::Id(Id(sys::SPA_DIRECTION_OUTPUT)),
                ),
                prop(sys::SPA_PARAM_ROUTE_device, Value::Int(4)),
                prop(sys::SPA_PARAM_ROUTE_save, Value::Bool(false)),
            ],
        );
        assert_eq!(
            decode_route(pod(&bytes)),
            Ok(ActiveRoute {
                index: 2,
                card_profile_device: 4,
                dir: PortDir::Out,
            })
        );
        let no_device = object(
            SpaTypes::ObjectParamRoute,
            ParamType::Route,
            vec![
                prop(sys::SPA_PARAM_ROUTE_index, Value::Int(2)),
                prop(
                    sys::SPA_PARAM_ROUTE_direction,
                    Value::Id(Id(sys::SPA_DIRECTION_INPUT)),
                ),
            ],
        );
        assert_eq!(
            decode_route(pod(&no_device)),
            Err(DecodeError::Missing("device"))
        );
    }

    #[test]
    fn process_latency_ns() {
        let bytes = object(
            SpaTypes::ObjectParamProcessLatency,
            ParamType::ProcessLatency,
            vec![
                prop(sys::SPA_PARAM_PROCESS_LATENCY_quantum, Value::Float(0.0)),
                prop(sys::SPA_PARAM_PROCESS_LATENCY_rate, Value::Int(0)),
                prop(sys::SPA_PARAM_PROCESS_LATENCY_ns, Value::Long(2_500_000)),
            ],
        );
        assert_eq!(decode_process_latency(pod(&bytes)), Ok(2_500_000));
        let wrong = object(
            SpaTypes::ObjectParamProcessLatency,
            ParamType::ProcessLatency,
            vec![prop(sys::SPA_PARAM_PROCESS_LATENCY_ns, Value::Int(5))],
        );
        assert_eq!(
            decode_process_latency(pod(&wrong)),
            Err(DecodeError::WrongType("ns"))
        );
    }

    #[test]
    fn truncated_pod_is_error_not_panic() {
        let full = stereo_props();
        let size = u32::from_ne_bytes(full[..4].try_into().unwrap());
        // The last property (channelMap, two ids) takes 32 bytes: key + flags,
        // array header, child header, two values. Shrinking the declared object size
        // by less than that cuts it somewhere in the middle.
        for cut in (4..32).step_by(4) {
            let mut bytes = full.clone();
            bytes[..4].copy_from_slice(&(size - cut).to_ne_bytes());
            assert!(
                decode_props(pod(&bytes)).is_err(),
                "cut of {cut} bytes decoded"
            );
        }
    }
}
