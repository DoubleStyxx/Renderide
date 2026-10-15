//! Host-driven OpenXR haptic feedback mapping.

use openxr as xr;

use crate::shared::{Chirality, HapticPointState, VRControllerOutputState, VROutputState};

use super::openxr_input::OpenxrInput;

const DEFAULT_FRAME_DELTA_SECONDS: f32 = 1.0 / 60.0;
const MAX_FRAME_DELTA_SECONDS: f32 = 0.25;
const CONTINUOUS_DURATION_SECONDS: f64 = 0.05;
const MAX_HAPTIC_DURATION_SECONDS: f64 = 10.0;
const ONE_SHOT_FREQUENCY_HZ: f32 = 20.0;
const TAU: f32 = core::f32::consts::PI * 2.0;

/// Stateful mapper from host `VROutputState` haptic fields to OpenXR feedback events.
#[derive(Debug)]
pub(crate) struct OpenxrHaptics {
    frame_delta_seconds: f32,
    pain_phi: f32,
    left: HandHapticSimulationState,
    right: HandHapticSimulationState,
}

impl Default for OpenxrHaptics {
    fn default() -> Self {
        Self {
            frame_delta_seconds: DEFAULT_FRAME_DELTA_SECONDS,
            pain_phi: 0.0,
            left: HandHapticSimulationState::new(0x4c45_4654),
            right: HandHapticSimulationState::new(0x5249_4748),
        }
    }
}

impl OpenxrHaptics {
    /// Updates the wall-clock delta used for pain-wave phase simulation.
    pub(crate) fn set_frame_delta_seconds(&mut self, seconds: f32) {
        self.frame_delta_seconds = if seconds.is_finite() {
            seconds.clamp(0.0, MAX_FRAME_DELTA_SECONDS)
        } else {
            DEFAULT_FRAME_DELTA_SECONDS
        };
    }

    /// Applies the latest host VR haptic output to the OpenXR haptic actions.
    pub(crate) fn apply_output_state(
        &mut self,
        input: &OpenxrInput,
        session: &xr::Session<xr::Vulkan>,
        output: Option<&VROutputState>,
    ) {
        profiling::scope!("xr::haptics");
        let left = output.and_then(|output| output.left_controller.as_ref());
        let right = output.and_then(|output| output.right_controller.as_ref());
        self.advance_pain_phase(left, right);
        let pain_phi = self.pain_phi;
        Self::apply_controller_output(
            input,
            session,
            Chirality::Left,
            left,
            pain_phi,
            &mut self.left,
        );
        Self::apply_controller_output(
            input,
            session,
            Chirality::Right,
            right,
            pain_phi,
            &mut self.right,
        );
    }

    fn advance_pain_phase(
        &mut self,
        left: Option<&VRControllerOutputState>,
        right: Option<&VRControllerOutputState>,
    ) {
        let left_pain = left
            .map(|controller| sanitize_unit(controller.haptic_state.pain))
            .unwrap_or(0.0);
        let right_pain = right
            .map(|controller| sanitize_unit(controller.haptic_state.pain))
            .unwrap_or(0.0);
        let max_pain = left_pain.max(right_pain);
        let pain_frequency = lerp(80.0 / 60.0, 140.0 / 60.0, max_pain);
        self.pain_phi =
            (self.pain_phi + TAU * self.frame_delta_seconds * pain_frequency).rem_euclid(TAU * 2.0);
    }

    fn apply_controller_output(
        input: &OpenxrInput,
        session: &xr::Session<xr::Vulkan>,
        side: Chirality,
        controller: Option<&VRControllerOutputState>,
        pain_phi: f32,
        state: &mut HandHapticSimulationState,
    ) {
        let commands = controller_feedback_commands(controller, pain_phi, state);

        if let Some(command) = commands.continuous {
            apply_feedback_command(input, session, side, command);
        }

        if let Some(command) = commands.one_shot {
            apply_feedback_command(input, session, side, command);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ControllerHapticCommands {
    continuous: Option<HapticFeedbackCommand>,
    one_shot: Option<HapticFeedbackCommand>,
}

fn controller_feedback_commands(
    controller: Option<&VRControllerOutputState>,
    pain_phi: f32,
    state: &mut HandHapticSimulationState,
) -> ControllerHapticCommands {
    let Some(controller) = controller else {
        return ControllerHapticCommands::default();
    };
    ControllerHapticCommands {
        continuous: continuous_feedback_command(controller.haptic_state, pain_phi, state),
        one_shot: one_shot_feedback_command(controller.vibrate_time),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HapticFeedbackCommand {
    duration_nanos: i64,
    frequency_hz: f32,
    amplitude: f32,
}

impl HapticFeedbackCommand {
    fn new(duration_seconds: f64, frequency_hz: f32, amplitude: f32) -> Option<Self> {
        if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
            return None;
        }
        if !frequency_hz.is_finite() || frequency_hz <= 0.0 {
            return None;
        }
        if !amplitude.is_finite() || amplitude <= f32::EPSILON {
            return None;
        }

        let clamped_duration_seconds = duration_seconds.clamp(0.0, MAX_HAPTIC_DURATION_SECONDS);
        let duration_nanos = (clamped_duration_seconds * 1_000_000_000.0).round() as i64;
        if duration_nanos <= 0 {
            return None;
        }

        Some(Self {
            duration_nanos,
            frequency_hz: frequency_hz.clamp(1.0, 400.0),
            amplitude: amplitude.clamp(0.0, 1.0),
        })
    }

    fn xr_duration(self) -> xr::Duration {
        xr::Duration::from_nanos(self.duration_nanos)
    }
}

#[derive(Debug)]
struct HandHapticSimulationState {
    temp_phi: f32,
    noise_seed: u32,
}

impl HandHapticSimulationState {
    const fn new(noise_seed: u32) -> Self {
        Self {
            temp_phi: 0.0,
            noise_seed,
        }
    }

    fn next_unit_noise(&mut self) -> f32 {
        self.noise_seed = self
            .noise_seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        ((self.noise_seed >> 8) as f32) / 16_777_215.0
    }
}

fn apply_feedback_command(
    input: &OpenxrInput,
    session: &xr::Session<xr::Vulkan>,
    side: Chirality,
    command: HapticFeedbackCommand,
) {
    if let Err(error) = input.apply_haptic_feedback(
        session,
        side,
        command.xr_duration(),
        command.frequency_hz,
        command.amplitude,
    ) {
        logger::trace!("OpenXR {side:?} haptic feedback failed: {error:?}");
    }
}

fn continuous_feedback_command(
    point: HapticPointState,
    pain_phi: f32,
    state: &mut HandHapticSimulationState,
) -> Option<HapticFeedbackCommand> {
    let force = sanitize_unit(point.force);
    let vibration = sanitize_unit(point.vibration);
    let pain = sanitize_unit(point.pain);
    let normalized_temperature = sanitize_unit(point.temperature.abs() / 100.0);

    let mut intensity = 0.0;
    let mut frequency = 0.0;
    let mut weight_sum = 0.0;

    intensity += force * force;
    frequency += lerp(20.0, 160.0, force) * force;
    weight_sum += force;

    let vibration_intensity = sanitize_unit(vibration * 20.0);
    intensity += vibration_intensity * vibration;
    frequency += lerp(5.0, 320.0, vibration) * vibration;
    weight_sum += vibration;

    let pain_wave = pain_phi.sin();
    let pain_pulse = pain_wave.abs().powf(0.25) * (pain_phi * 0.5).sin().signum().max(0.0);
    let pain_root = pain.powf(0.25);
    let pain_amplitude = pain_pulse * pain_root + state.next_unit_noise() * pain_root * 0.2;
    intensity += pain_amplitude * pain;
    frequency += lerp(60.0 + pain_pulse * 80.0, 80.0 + pain_pulse * 120.0, pain) * pain;
    weight_sum += pain;

    state.temp_phi = (state.temp_phi + normalized_temperature * 4.0).rem_euclid(20_000.0);
    let temperature_amplitude = normalized_temperature * value_noise_1d(state.temp_phi);
    intensity += temperature_amplitude * normalized_temperature;
    frequency += lerp(5.0, 200.0, normalized_temperature) * normalized_temperature;
    weight_sum += normalized_temperature;

    if intensity <= f32::EPSILON || weight_sum <= f32::EPSILON {
        return None;
    }

    HapticFeedbackCommand::new(
        CONTINUOUS_DURATION_SECONDS,
        frequency / weight_sum,
        intensity / weight_sum,
    )
}

fn one_shot_feedback_command(vibrate_time: f64) -> Option<HapticFeedbackCommand> {
    HapticFeedbackCommand::new(vibrate_time, ONE_SHOT_FREQUENCY_HZ, 1.0)
}

fn sanitize_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn value_noise_1d(x: f32) -> f32 {
    let cell = x.floor() as i32;
    let t = x - cell as f32;
    let smoothed = t * t * (3.0 - 2.0 * t);
    lerp(hash_unit(cell), hash_unit(cell + 1), smoothed)
}

fn hash_unit(x: i32) -> f32 {
    let mut n = x as u32;
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846c_a68b);
    n ^= n >> 16;
    (n as f32) / (u32::MAX as f32)
}

#[cfg(test)]
mod tests {
    use super::{
        CONTINUOUS_DURATION_SECONDS, HandHapticSimulationState, HapticFeedbackCommand,
        ONE_SHOT_FREQUENCY_HZ, continuous_feedback_command, controller_feedback_commands,
        one_shot_feedback_command,
    };
    use crate::shared::HapticPointState;

    #[test]
    fn default_haptic_point_produces_no_continuous_feedback() {
        let mut state = HandHapticSimulationState::new(1);

        let command = continuous_feedback_command(HapticPointState::default(), 0.0, &mut state);

        assert_eq!(command, None);
    }

    #[test]
    fn force_maps_to_expected_frequency_and_bounded_amplitude() {
        let mut state = HandHapticSimulationState::new(1);

        let command = continuous_feedback_command(
            HapticPointState {
                force: 0.5,
                ..HapticPointState::default()
            },
            0.0,
            &mut state,
        )
        .expect("force haptics should produce feedback");

        assert_eq!(
            command.duration_nanos,
            (CONTINUOUS_DURATION_SECONDS * 1_000_000_000.0).round() as i64
        );
        assert!((command.frequency_hz - 90.0).abs() < f32::EPSILON);
        assert!((command.amplitude - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn vibration_and_temperature_are_bounded() {
        let mut state = HandHapticSimulationState::new(7);

        let command = continuous_feedback_command(
            HapticPointState {
                vibration: 0.25,
                temperature: 500.0,
                ..HapticPointState::default()
            },
            0.0,
            &mut state,
        )
        .expect("vibration haptics should produce feedback");

        assert!((5.0..=320.0).contains(&command.frequency_hz));
        assert!((0.0..=1.0).contains(&command.amplitude));
    }

    #[test]
    fn pain_maps_to_bounded_feedback() {
        let mut state = HandHapticSimulationState::new(11);

        let command = continuous_feedback_command(
            HapticPointState {
                pain: 1.0,
                ..HapticPointState::default()
            },
            core::f32::consts::PI,
            &mut state,
        )
        .expect("pain haptics should produce feedback");

        assert!((60.0..=200.0).contains(&command.frequency_hz));
        assert!((0.0..=1.0).contains(&command.amplitude));
    }

    #[test]
    fn one_shot_vibration_uses_full_strength_twenty_hz() {
        let command = one_shot_feedback_command(0.03).expect("positive vibration should map");

        assert_eq!(
            command,
            HapticFeedbackCommand {
                duration_nanos: 30_000_000,
                frequency_hz: ONE_SHOT_FREQUENCY_HZ,
                amplitude: 1.0,
            }
        );
    }

    #[test]
    fn invalid_one_shot_vibration_is_ignored() {
        assert_eq!(one_shot_feedback_command(0.0), None);
        assert_eq!(one_shot_feedback_command(-1.0), None);
        assert_eq!(one_shot_feedback_command(f64::NAN), None);
    }

    #[test]
    fn missing_controller_output_produces_no_feedback_commands() {
        let mut state = HandHapticSimulationState::new(13);

        let commands = controller_feedback_commands(None, 1.0, &mut state);

        assert_eq!(commands.continuous, None);
        assert_eq!(commands.one_shot, None);
    }
}
