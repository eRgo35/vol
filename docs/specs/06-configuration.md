# 06 — Configuration tab

Area prefix `VOL-CFG`. Shared: [02 §2.3 rows/selection, §2.5 popups](02-playback.md).

## 1. pavucontrol → vol mapping

| pavucontrol | vol |
|---|---|
| Configuration tab: one box per card with icon, name, **Profile** combo; unavailable profiles marked | one 3-line row per `Audio/Device` object; `Profile: <desc> ▾`; popup to choose |
| Card = PulseAudio card | Device object with `media.class = "Audio/Device"` (observed: ALSA cards, `device.api = alsa`; Bluetooth cards use the same class) |
| Profile = card profile | Device `Profile` param (current) / `EnumProfile` (choices) |
| "Show: …" filter on this tab (e.g. hide unused/virtual cards) | **Deferred**. v1 lists every `Audio/Device`; the filter bar shows only the count |
| Codec choice for Bluetooth | **Deferred** (device-level `Props` codec properties unverified) |

## 2. Row population

`view::card_rows(&Store) -> Vec<CardRow>`:

```rust
pub struct CardRow {
    pub key: String,                    // props["device.name"]
    pub id: ObjectId, pub serial: Serial,
    pub title: String,
    pub active: Option<Profile>,        // from Device.active_profile -> profiles
    pub choices: Vec<Profile>,          // sorted (priority desc, index asc)
}
```

| Field | Source | Rule |
|---|---|---|
| `title` | `device.description` | → `device.nick` → `device.name` |
| active profile text | `EnumProfile[index == Profile.index].description` | `Profile: <description>`; `available == No` → ` (unavailable)`; no `Profile` param yet → `Profile: …`; index absent from `EnumProfile` → `Profile: #<index>` |
| `choices` | `EnumProfile` objects: `index`, `name`, `description`, `priority`, `available` | sorted by `priority` desc, `index` asc (observed: `off` has priority 0, so it lands last) |

Only devices whose `media.class == "Audio/Device"` are stored; devices with empty `EnumProfile` are still listed but their popup opens with status `"No profiles"` (no popup).

Observed (PipeWire 1.6.9): 4 cards (`alsa_card.pci-0000_28_00.4`, …); card 47 `EnumProfile` = `off`, `output:analog-stereo+input:analog-stereo` ("Analog Stereo Duplex"), `output:analog-stereo` ("Analog Stereo Output"), … with `Profile.index = 2`.

## 3. Layout

Row = 3 lines: `sel title`, `sel ␠␠␠ Profile: … ▾`, blank. Selected row has `▌` on its first two lines. No gauges.
```
 Playback │ Recording │ Output Devices │ Input Devices │ Configuration          
 Cards                                                                3 cards   
▌ Starship/Matisse HD Audio Controller                                          
▌   Profile: Analog Stereo Output ▾                                             
                                                                                
  CA0132 Sound Core3D                                                           
    Profile: Analog Stereo Duplex ▾                                             
                                                                                
  USB PnP Audio Device                                                          
    Profile: Mono Input (unavailable) ▾                                         
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
 ● PipeWire 1.6.9                                                │ ? help       
```
After `p`/`Enter`:
```
 Playback │ Recording │ Output Devices │ Input Devices │ Configuration          
 Cards                                                                3 cards   
▌ Starship/Matisse HD Audio Controller                                          
▌   Profile: Analog Stereo Output ▾                                             
                                                                                
  CA0132 Sound Core3D                                                           
    Profile: Analog Stereo Duplex ▾                                             
                                                                                
  USB PnP ┌─ Profile — Starship/Matisse HD Audio Controller ─────────┐          
    Profil│  Analog Stereo Duplex                                    │          
          │▶ Analog Stereo Output  ✓                                 │          
          │  Analog Stereo Input                                     │          
          │  Analog Surround 5.1 Output + Analog Stereo Input        │          
          │  Off                                                     │          
          └──────────────────────────────────────────────────────────┘          
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
 ● PipeWire 1.6.9                                                │ ? help       
```

## 4. Setting a profile

`Command::SetProfile{device, index}` →
```
Device::set_param(ParamType::Profile, 0,
  Object(SPA_TYPE_OBJECT_ParamProfile, SPA_PARAM_Profile,
    Property(SPA_PARAM_PROFILE_index, Int(index)),
    Property(SPA_PARAM_PROFILE_save,  Bool(true))))
```
VERIFY: `pw-cli set-param <device-id> Profile '{ index: <i>, save: true }'`; the `Profile` param echoes `index`; sink/source nodes of the card appear/disappear in the Output/Input tabs.

After a profile change, the card's old nodes are removed and new ones added by the daemon; vol does nothing special (live registry updates, VOL-ARCH-022).

## 5. Requirements

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-CFG-001 | MUST | Tab renders one row per `Audio/Device` per §2–§3 in `Serial` order. | Stub |
| VOL-CFG-002 | MUST | Filter bar shows no filter name; right side shows `<n> cards` (or `<sel>/<total>` when scrolling). `f`/`F` give status `"No filters on this tab"`. | Missing |
| VOL-CFG-003 | MUST | `p`/`Enter` opens "Profile — `<title>`" with `choices`; entries with `available == No` are shown with ` (unavailable)` and dimmed but selectable (selecting emits status `"<profile> is unavailable"`); `✓` on current. | Missing |
| VOL-CFG-004 | MUST | Apply → `Command::SetProfile` per §4; row text updates from the echoed `Profile` param, not optimistically. | Missing |
| VOL-CFG-005 | MUST | Empty state: `No audio cards` / `PipeWire reports no audio devices.` | Missing |
| VOL-CFG-006 | MUST | Card removed while the popup is open → popup closes, status `"Card removed"`; command to a removed device → `CommandFailed{DeviceGone}` → same status. | Missing |
| VOL-CFG-007 | MUST | Volume/mute/lock/default keys on this tab are no-ops with status `"Not applicable here"`. | Missing |
| VOL-CFG-008 | SHOULD | Selection `RowKey = device.name` survives profile changes (the `Device` object persists while its nodes churn). | Missing |
| VOL-CFG-009 | MAY | "Show: All Devices" filter (pavucontrol parity) and Bluetooth codec selection: **deferred**. | Missing |
