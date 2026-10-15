//! Shared unit-box volume ray helpers.

#define_import_path renderide::material::volume_box

#import renderide::draw::types as dt
#import renderide::frame::globals as rg

struct LocalSegment {
    start: vec3<f32>,
    end: vec3<f32>,
    camera: vec3<f32>,
    dir: vec3<f32>,
    distance: f32,
}

struct WorldSegment {
    start: vec3<f32>,
    end: vec3<f32>,
    camera: vec3<f32>,
    dir: vec3<f32>,
    distance: f32,
}

const UNIT_BOX_HALF_EXTENT: f32 = 0.5;
const VOLUME_EPSILON: f32 = 0.000001;
const VOLUME_BIG_DISTANCE: f32 = 1000000.0;

fn distance_sqr(a: vec3<f32>, b: vec3<f32>) -> f32 {
    let v = a - b;
    return dot(v, v);
}

fn safe_normalize_or(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if (len <= VOLUME_EPSILON) {
        return fallback;
    }
    return v / len;
}

fn safe_divisor(v: f32) -> f32 {
    if (abs(v) <= VOLUME_EPSILON) {
        return select(-VOLUME_EPSILON, VOLUME_EPSILON, v >= 0.0);
    }
    return v;
}

fn clamp_volume_source_rgb(color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
}

fn inside_unit_cube(pos: vec3<f32>) -> bool {
    return all(abs(pos) <= vec3<f32>(UNIT_BOX_HALF_EXTENT));
}

fn ray_axis_interval(origin: f32, dir: f32) -> vec2<f32> {
    if (abs(dir) <= VOLUME_EPSILON) {
        if (abs(origin) > UNIT_BOX_HALF_EXTENT) {
            return vec2<f32>(1.0, -1.0);
        }
        return vec2<f32>(-VOLUME_BIG_DISTANCE, VOLUME_BIG_DISTANCE);
    }

    let t0 = (-UNIT_BOX_HALF_EXTENT - origin) / dir;
    let t1 = (UNIT_BOX_HALF_EXTENT - origin) / dir;
    return vec2<f32>(min(t0, t1), max(t0, t1));
}

fn ray_unit_cube_interval(origin: vec3<f32>, dir: vec3<f32>) -> vec2<f32> {
    let x = ray_axis_interval(origin.x, dir.x);
    let y = ray_axis_interval(origin.y, dir.y);
    let z = ray_axis_interval(origin.z, dir.z);
    return vec2<f32>(max(max(x.x, y.x), z.x), min(min(x.y, y.y), z.y));
}

fn intersect_unit_cube(origin: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    let interval = ray_unit_cube_interval(origin, dir);
    if (interval.x > interval.y) {
        return origin;
    }
    return origin + dir * interval.x;
}

fn clamp_inside_unit_cube(pos: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    if (inside_unit_cube(pos)) {
        return pos;
    }
    return intersect_unit_cube(pos, dir);
}

fn world_to_object_position(draw: dt::PerDrawUniforms, world_pos: vec3<f32>) -> vec3<f32> {
    let local_from_world_linear = transpose(draw.normal_matrix);
    return local_from_world_linear * (world_pos - draw.model[3].xyz);
}

fn object_to_world_position(draw: dt::PerDrawUniforms, local_pos: vec3<f32>) -> vec3<f32> {
    return (draw.model * vec4<f32>(local_pos, 1.0)).xyz;
}

fn local_camera_position(draw: dt::PerDrawUniforms, view_layer: u32) -> vec3<f32> {
    return world_to_object_position(draw, rg::camera_world_pos_for_view(view_layer));
}

fn object_scale(draw: dt::PerDrawUniforms) -> vec3<f32> {
    return vec3<f32>(
        length(draw.model[0].xyz),
        length(draw.model[1].xyz),
        length(draw.model[2].xyz),
    );
}

fn depth_end_ratio(scene_depth: f32, part_depth: f32) -> f32 {
    return min(scene_depth / max(part_depth, VOLUME_EPSILON), 1.0);
}

fn local_depth_limited_segment(
    draw: dt::PerDrawUniforms,
    local_back_face: vec3<f32>,
    scene_depth: f32,
    part_depth: f32,
    view_layer: u32,
) -> LocalSegment {
    let camera = local_camera_position(draw, view_layer);
    let dir = safe_normalize_or(local_back_face - camera, vec3<f32>(0.0, 0.0, 1.0));
    let start = clamp_inside_unit_cube(camera, dir);
    let max_dist = distance(camera, local_back_face);
    let end = camera + dir * max_dist * depth_end_ratio(scene_depth, part_depth);
    let dist = distance(start, end);
    return LocalSegment(start, end, camera, dir, dist);
}

fn world_depth_limited_segment(
    camera_world: vec3<f32>,
    world_back_face: vec3<f32>,
    scene_depth: f32,
) -> WorldSegment {
    let dir = safe_normalize_or(world_back_face - camera_world, vec3<f32>(0.0, 0.0, 1.0));
    let end = camera_world + dir * scene_depth;
    return WorldSegment(camera_world, end, camera_world, dir, distance(camera_world, end));
}
