use hashbrown::HashMap;
use serde::Deserialize;

use super::types::{
    ActionManifest, ActionManifestRaw, ActionType, BindingEntry, BindingProfile, ExtensionGate,
    Manifest, ManifestError,
};

/// Parses `actions.toml` source text and validates the action inventory.
pub fn parse_action_manifest(toml_src: &str) -> Result<ActionManifest, ManifestError> {
    let raw: ActionManifestRaw = toml::from_str(toml_src).map_err(ManifestError::ActionsToml)?;
    ActionManifest::from_parsed(raw)
}

/// Raw per-profile binding file (see `bindings/*.toml`).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct BindingProfileRaw {
    profile: String,
    #[serde(default)]
    extension_gate: Option<String>,
    #[serde(default)]
    binding: Vec<BindingEntryRaw>,
}

/// Raw `[[binding]]` entry with stringly typed extension metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct BindingEntryRaw {
    action: String,
    path: String,
    #[serde(default)]
    extension_gate: Option<String>,
}

/// Resolves a TOML `extension_gate` string for either profile-level or binding-level metadata.
fn parse_extension_gate(
    profile: &str,
    name: Option<String>,
) -> Result<Option<ExtensionGate>, ManifestError> {
    let Some(name) = name else {
        return Ok(None);
    };
    match ExtensionGate::from_str(&name) {
        Some(gate) => Ok(Some(gate)),
        None => Err(ManifestError::UnknownExtensionGate {
            profile: profile.to_string(),
            gate: name,
        }),
    }
}

/// Parses a single profile file, without validating against an action manifest.
fn parse_binding_profile(
    file_label: &str,
    toml_src: &str,
) -> Result<BindingProfile, ManifestError> {
    let raw: BindingProfileRaw =
        toml::from_str(toml_src).map_err(|e| ManifestError::BindingToml {
            file: file_label.to_string(),
            source: e,
        })?;
    let extension_gate = parse_extension_gate(&raw.profile, raw.extension_gate)?;
    let mut bindings = Vec::with_capacity(raw.binding.len());
    for binding in raw.binding {
        bindings.push(BindingEntry {
            action: binding.action,
            path: binding.path,
            extension_gate: parse_extension_gate(&raw.profile, binding.extension_gate)?,
        });
    }
    Ok(BindingProfile {
        profile: raw.profile,
        extension_gate,
        bindings,
    })
}

/// Returns `true` when `path` ends with a component denoting a haptic output.
fn is_haptic_path(path: &str) -> bool {
    path.ends_with("/output/haptic")
}

/// Checks that every binding in `profile` resolves to a known action and that haptic actions
/// bind only to haptic output paths (and vice versa).
fn validate_profile(
    actions: &ActionManifest,
    profile: &BindingProfile,
) -> Result<(), ManifestError> {
    for binding in &profile.bindings {
        let ty =
            actions
                .action_type(&binding.action)
                .ok_or_else(|| ManifestError::UnknownAction {
                    profile: profile.profile.clone(),
                    action: binding.action.clone(),
                })?;
        let path_is_haptic = is_haptic_path(&binding.path);
        if ty == ActionType::Haptic && !path_is_haptic {
            return Err(ManifestError::HapticOnWrongPath {
                profile: profile.profile.clone(),
                action: binding.action.clone(),
                path: binding.path.clone(),
            });
        }
        if ty != ActionType::Haptic && path_is_haptic {
            return Err(ManifestError::NonHapticOnHapticPath {
                profile: profile.profile.clone(),
                action: binding.action.clone(),
                path: binding.path.clone(),
            });
        }
    }
    Ok(())
}

/// Builds a validated [`Manifest`] from parsed actions and a list of parsed profile files.
///
/// Each element of `profile_sources` is `(file_label, file_contents)`. The label is surfaced in
/// diagnostics to point at whichever TOML file contains a validation failure.
pub fn build_manifest(
    actions_src: &str,
    profile_sources: &[(&str, &str)],
) -> Result<Manifest, ManifestError> {
    let actions = parse_action_manifest(actions_src)?;
    let mut profiles = Vec::with_capacity(profile_sources.len());
    let mut seen_profile_paths: HashMap<String, usize> = HashMap::new();

    for (label, src) in profile_sources {
        let profile = parse_binding_profile(label, src)?;
        if seen_profile_paths
            .insert(profile.profile.clone(), profiles.len())
            .is_some()
        {
            return Err(ManifestError::DuplicateProfile(profile.profile));
        }
        validate_profile(&actions, &profile)?;
        profiles.push(profile);
    }

    Ok(Manifest { actions, profiles })
}
